# Proves an installed copy updates itself, the way a person's copy would.
#
#   hop A  The release the README links to today is installed, and its
#          app-update.yml is pointed at a local feed serving build A of this
#          commit. Nothing in that old version knows it is being tested: it
#          has to find A, download it, verify the sha512, and (if a polite
#          close from outside reaches it) install A on quit.
#   hop B  Build A, installed, is pointed at build B through AEVAL_UPDATE_FEED
#          and told to quit once B is ready. B must be installed afterwards,
#          and the updater's own log must hold a sha512 matching B's latest.yml.
#   hop C  B, through the smoke test: against a feed serving itself it must
#          say "current"; against a feed that refuses connections it must be
#          retrying quietly, not erroring.
#
# Meant for a throwaway Windows machine: .github/workflows/update-check.yml
# runs it on a GitHub runner. On your own desktop it would replace your copy.
#
#   ./tools/update-check.ps1 -FeedA dist-a -VersionA 1.1.2 -FeedB dist-b -VersionB 1.1.3 -Out out

param(
  [Parameter(Mandatory)] [string] $FeedA,
  [Parameter(Mandatory)] [string] $VersionA,
  [Parameter(Mandatory)] [string] $FeedB,
  [Parameter(Mandatory)] [string] $VersionB,
  [Parameter(Mandatory)] [string] $Out,
  [string] $Repo = 'aevalmere/aevalbuddy',
  [int] $Port = 8765
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$FeedA = (Resolve-Path $FeedA).Path
$FeedB = (Resolve-Path $FeedB).Path
$onRunner = [bool]$env:GITHUB_ACTIONS
$scratch = Join-Path $(if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }) 'aeval-update'
New-Item -ItemType Directory -Force $scratch | Out-Null
$feedUrl = "http://127.0.0.1:$Port"

$checks = [System.Collections.Generic.List[object]]::new()
function Check([string]$name, [bool]$ok, [string]$detail = '', [switch]$Soft) {
  $level = if ($ok) { 'ok' } elseif ($Soft) { 'warn' } else { 'FAIL' }
  $checks.Add([pscustomobject]@{ Name = $name; Ok = $ok; Soft = [bool]$Soft; Detail = $detail; Level = $level })
  Write-Host "[$level] $name $detail"
}

function Annotate([string]$level, [string]$title, [string]$message) {
  if (-not $onRunner) { return }
  $m = $message.Replace('%', '%25').Replace("`r", '%0D').Replace("`n", '%0A')
  $t = $title.Replace('%', '%25').Replace(':', '%3A').Replace(',', '%2C')
  Write-Host "::${level} title=${t}::$m"
}

trap {
  $at = $_.InvocationInfo
  Annotate 'error' 'update-check.ps1' "line $($at.ScriptLineNumber): $($_.Exception.Message)`n$($at.Line.Trim())"
  exit 1
}

function Sha512([string]$path) {
  $s = [IO.File]::OpenRead($path)
  try { [Convert]::ToBase64String([Security.Cryptography.SHA512]::Create().ComputeHash($s)) } finally { $s.Dispose() }
}

function Yml([string]$text, [string]$key) {
  [regex]::Match($text, "(?m)^${key}:\s*'?([^'\r\n]+)'?").Groups[1].Value.Trim()
}

function Find-Uninstall {
  Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'AevalBuddy*' } |
    Select-Object -First 1
}

function Installed-Version {
  $e = Find-Uninstall
  if ($e) { $e.DisplayVersion } else { '' }
}

function Wait-Until([scriptblock]$cond, [int]$seconds) {
  $deadline = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $deadline) {
    if (& $cond) { return $true }
    Start-Sleep -Seconds 2
  }
  [bool](& $cond)
}

function Install([string]$installer) {
  $p = Start-Process -FilePath $installer -ArgumentList '/S' -PassThru -Wait
  $p.ExitCode
}

# Waits for every installer process to finish: an update spawned by the app
# keeps running after the app itself has exited.
function Wait-Installers([int]$seconds) {
  Wait-Until { -not (Get-Process 'AevalBuddy-Setup', 'installer' -ErrorAction SilentlyContinue) } $seconds | Out-Null
}

function Start-App([string]$exe, [string]$label, [hashtable]$vars) {
  $saved = @{}
  foreach ($k in $vars.Keys) {
    $saved[$k] = [Environment]::GetEnvironmentVariable($k)
    [Environment]::SetEnvironmentVariable($k, $vars[$k])
  }
  try {
    $p = Start-Process -FilePath $exe -PassThru `
      -RedirectStandardOutput (Join-Path $Out "$label.log") `
      -RedirectStandardError (Join-Path $Out "$label.stderr.log")
    $null = $p.Handle
    $p
  } finally {
    foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
  }
}

function App-Text([string]$label) {
  "$(Get-Content (Join-Path $Out "$label.log") -Raw -ErrorAction SilentlyContinue)$(Get-Content (Join-Path $Out "$label.stderr.log") -Raw -ErrorAction SilentlyContinue)"
}

# The feed log is append-only, so "requests during this hop" is every line
# after the count taken when the hop started.
function Feed-Mark {
  if (Test-Path $feedLog) { @(Get-Content $feedLog).Count } else { 0 }
}

function Feed-Requests([int]$since) {
  if (-not (Test-Path $feedLog)) { return @() }
  @(Get-Content $feedLog | Select-Object -Skip $since | ForEach-Object { $_ | ConvertFrom-Json })
}

# How the exe arrived: ranges of it means differential, one 200 means full.
function Describe-Download([string]$version, [int]$since) {
  $reqs = Feed-Requests $since
  $exe = @($reqs | Where-Object { $_.path -eq "/v$version/AevalBuddy-Setup.exe" })
  $ranges = @($exe | Where-Object { $_.status -eq 206 })
  $full = @($exe | Where-Object { $_.status -eq 200 -and $_.method -eq 'GET' })
  $bytes = ($exe | Measure-Object -Property bytes -Sum).Sum
  $oldMap = @($reqs | Where-Object { $_.path -like '*.blockmap' -and $_.path -notlike "/v$version/*" })
  $kind = if ($full.Count) { 'full' } elseif ($ranges.Count) { 'differential' } else { 'none' }
  [pscustomobject]@{
    Kind = $kind
    Text = "$kind download of $version`: $($ranges.Count) range requests, $($full.Count) full GETs, $([math]::Round($bytes / 1MB, 1)) MB of the exe sent; old blockmap requested: $($oldMap.path -join ', ')"
  }
}

# GitHub's release download links answer 5xx now and then (a 504 on
# latest/download/latest.yml failed a whole run in Nov 2026). A 5xx or a
# timeout is asked again after a pause; anything else throws at once, and a
# 5xx that lasts through every attempt throws the last error.
function Get-Release([string]$url, [string]$outFile) {
  $pauses = @(10, 30, 60)
  for ($n = 0; ; $n++) {
    try {
      Invoke-WebRequest $url -OutFile $outFile -TimeoutSec 300
      return
    } catch {
      $code = 0
      if ($_.Exception.Response) { $code = [int]$_.Exception.Response.StatusCode }
      $timedOut = $_.Exception -is [System.Threading.Tasks.TaskCanceledException] -or
        $_.Exception.InnerException -is [System.TimeoutException] -or
        $_.Exception.Message -match 'timed out|timeout'
      if ($n -ge $pauses.Count -or -not ($code -ge 500 -or $timedOut)) { throw }
      Write-Host "[retry] $url answered $(if ($code) { $code } else { 'a timeout' }); again in $($pauses[$n])s"
      Start-Sleep -Seconds $pauses[$n]
    }
  }
}

$localApp = $env:LOCALAPPDATA

# ------------------------------------------------------------ the old release

$prevDir = Join-Path $scratch 'prev'
New-Item -ItemType Directory -Force $prevDir | Out-Null
$base = "https://github.com/$Repo/releases"
Get-Release "$base/latest/download/latest.yml" "$prevDir\latest.yml"
Get-Release "$base/latest/download/AevalBuddy-Setup.exe" "$prevDir\AevalBuddy-Setup.exe"
$prevYml = Get-Content "$prevDir\latest.yml" -Raw
$prev = Yml $prevYml 'version'
Get-Release "$base/download/v$prev/AevalBuddy-Setup.exe.blockmap" "$prevDir\AevalBuddy-Setup.exe.blockmap"
Check 'release.latestYmlPath' ((Yml $prevYml 'path') -eq 'AevalBuddy-Setup.exe') "v$prev latest.yml path=$(Yml $prevYml 'path')"
Check 'release.sha512' ((Sha512 "$prevDir\AevalBuddy-Setup.exe") -eq (Yml $prevYml 'sha512')) "v$prev exe matches its latest.yml"
Check 'versions.order' ([version]$VersionA -gt [version]$prev -and [version]$VersionB -gt [version]$VersionA) "$prev < $VersionA < $VersionB"

# ------------------------------------------------------------------ the feed

$feed = Join-Path $scratch 'feed'
Remove-Item -Recurse -Force $feed -ErrorAction SilentlyContinue
foreach ($pair in @(@($prev, $prevDir, $false), @($VersionA, $FeedA, $true), @($VersionB, $FeedB, $true))) {
  $dir = Join-Path $feed "v$($pair[0])"
  New-Item -ItemType Directory -Force $dir | Out-Null
  Copy-Item (Join-Path $pair[1] 'AevalBuddy-Setup.exe.blockmap') $dir
  if ($pair[2]) {
    Copy-Item (Join-Path $pair[1] 'AevalBuddy-Setup.exe'), (Join-Path $pair[1] 'latest.yml') $dir
  }
}
$ymlA = Get-Content "$feed\v$VersionA\latest.yml" -Raw
$ymlB = Get-Content "$feed\v$VersionB\latest.yml" -Raw
Check 'build.versionA' ((Yml $ymlA 'version') -eq $VersionA) "latest.yml says $(Yml $ymlA 'version')"
Check 'build.versionB' ((Yml $ymlB 'version') -eq $VersionB) "latest.yml says $(Yml $ymlB 'version')"
Check 'build.versionlessName' ((Yml $ymlB 'path') -eq 'AevalBuddy-Setup.exe') "path=$(Yml $ymlB 'path')"

$feedLog = Join-Path $Out 'feed-requests.jsonl'
$server = Start-Process node -ArgumentList 'tools/update-feed.js', "`"$feed`"", $Port, "`"$feedLog`"" -PassThru -NoNewWindow `
  -RedirectStandardOutput (Join-Path $Out 'feed.log') -RedirectStandardError (Join-Path $Out 'feed.stderr.log')
$up = Wait-Until { try { (Invoke-WebRequest "$feedUrl/v$VersionA/latest.yml" -UseBasicParsing).StatusCode -eq 200 } catch { $false } } 30
Check 'feed.up' $up $feedUrl
if (-not $up) { exit 1 }

try {
  # --------------------------------------------------------------- hop A
  $code = Install "$prevDir\AevalBuddy-Setup.exe"
  Check 'hopA.install' ($code -eq 0 -and (Installed-Version) -eq $prev) "code=$code registered=$(Installed-Version)"
  $entry = Find-Uninstall
  $installDir = Split-Path ([regex]::Match($entry.UninstallString, '^"([^"]+)"').Groups[1].Value)
  $exe = Join-Path $installDir 'AevalBuddy.exe'
  $updateYml = Join-Path $installDir 'resources\app-update.yml'
  $shipped = Get-Content $updateYml -Raw
  Annotate 'notice' "app-update.yml shipped in v$prev" $shipped
  Check 'hopA.shippedFeed' ((Yml $shipped 'provider') -eq 'github' -and "$(Yml $shipped 'owner')/$(Yml $shipped 'repo')" -eq $Repo) ($shipped -replace '\s+', ' ')
  $cacheName = Yml $shipped 'updaterCacheDirName'
  $cache = Join-Path $localApp $cacheName
  Check 'hopA.installerCached' (Test-Path "$cache\installer.exe") "$cache\installer.exe is the differential download's old file" -Soft

  # The one change made to the old version: where it looks. Same cache
  # folder, a feed laid out like GitHub's, single ranges only.
  @(
    'provider: generic'
    "url: $feedUrl/v$VersionA/"
    'useMultipleRangeRequest: false'
    "updaterCacheDirName: $cacheName"
  ) -join "`n" | Set-Content $updateYml -Encoding ascii -NoNewline
  Remove-Item -Recurse -Force "$cache\pending" -ErrorAction SilentlyContinue

  $since = Feed-Mark
  $app = Start-App $exe 'hopA' @{ AEVAL_USERDATA = (Join-Path $scratch 'hopA') }
  # electron-updater renames the download into pending\ only after its sha512
  # matched latest.yml, then writes update-info.json beside it.
  $info = "$cache\pending\update-info.json"
  $got = Wait-Until { (Test-Path $info) -and (Test-Path "$cache\pending\AevalBuddy-Setup.exe") } 300
  Check 'hopA.downloaded' $got "v$prev found and downloaded $VersionA into $cache\pending"
  if ($got) {
    $hash = Sha512 "$cache\pending\AevalBuddy-Setup.exe"
    Check 'hopA.sha512' ($hash -eq (Yml $ymlA 'sha512')) "pending exe sha512 matches the $VersionA latest.yml"
    $recorded = (Get-Content $info -Raw | ConvertFrom-Json).sha512
    Check 'hopA.updateInfo' ($recorded -eq (Yml $ymlA 'sha512')) 'update-info.json records the same sha512'
  }
  $dl = Describe-Download $VersionA $since
  Annotate 'notice' "hop A: $prev -> $VersionA" $dl.Text
  Check 'hopA.transfer' ($dl.Kind -ne 'none') $dl.Text

  # v$prev has no test hook, so the only way to ask it to quit is the
  # polite one: WM_CLOSE to its windows, which ends in app.quit() and the
  # install-on-quit handler. Soft, because whether a tool window honours a
  # close from another process is Windows' call, not the app's.
  taskkill /PID $app.Id | Out-Null
  $exited = $app.WaitForExit(60000)
  $onQuit = $exited -and (Wait-Until { (Installed-Version) -eq $VersionA } 180)
  Check 'hopA.installOnQuit' $onQuit "after a polite close, registered=$(Installed-Version)" -Soft
  if (-not $onQuit) {
    Get-Process AevalBuddy -ErrorAction SilentlyContinue | Stop-Process -Force
    Wait-Installers 120
    $code = Install "$feed\v$VersionA\AevalBuddy-Setup.exe"
    Write-Host "installed $VersionA by hand: code=$code"
  }
  Wait-Installers 120
  Check 'hopA.result' ((Installed-Version) -eq $VersionA) "registered=$(Installed-Version)"
  $errText = App-Text 'hopA'
  if ($errText.Trim()) { Annotate 'notice' "hop A app output (v$prev)" $errText }

  # --------------------------------------------------------------- hop B
  $exeVersion = (Get-Item $exe).VersionInfo.ProductVersion
  Check 'hopB.startVersion' ($exeVersion -in $VersionA, "$VersionA.0") "exe=$exeVersion"
  Remove-Item -Recurse -Force "$cache\pending" -ErrorAction SilentlyContinue
  $userB = Join-Path $scratch 'hopB'
  $since = Feed-Mark
  $app = Start-App $exe 'hopB' @{
    AEVAL_USERDATA    = $userB
    AEVAL_UPDATE_FEED = "$feedUrl/v$VersionB/"
    AEVAL_UPDATE_QUIT = '1'
  }
  $exited = $app.WaitForExit(300000)
  Check 'hopB.quit' $exited 'the app quit by itself once the update was ready'
  if (-not $exited) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
  $installed = Wait-Until { (Installed-Version) -eq $VersionB } 180
  Wait-Installers 120
  Check 'hopB.installOnQuit' $installed "registered=$(Installed-Version) expected=$VersionB"
  $exeVersion = (Get-Item $exe).VersionInfo.ProductVersion
  Check 'hopB.exeVersion' ($exeVersion -in $VersionB, "$VersionB.0") "exe=$exeVersion"
  $log = Get-Content (Join-Path $userB 'updater.log') -Raw -ErrorAction SilentlyContinue
  Copy-Item (Join-Path $userB 'updater.log') (Join-Path $Out 'hopB-updater.log') -ErrorAction SilentlyContinue
  $line = [regex]::Match("$log", "downloaded $([regex]::Escape($VersionB)) sha512=(\S+) expected=(\S+) match=(\w+)")
  Check 'hopB.sha512' ($line.Success -and $line.Groups[1].Value -eq (Yml $ymlB 'sha512') -and $line.Groups[3].Value -eq 'true') "updater.log: $($line.Value)"
  Check 'hopB.noCrash' (-not (Test-Path (Join-Path $userB 'errors.log'))) 'no unhandled error during check, download, or install'
  $dl = Describe-Download $VersionB $since
  Annotate 'notice' "hop B: $VersionA -> $VersionB" $dl.Text
  if ($log) { Annotate 'notice' 'hop B updater.log' $log }

  # --------------------------------------------------------------- hop C
  $smoke = Start-App $exe 'hopC-current' @{
    AEVAL_SMOKE         = '1'
    AEVAL_USERDATA      = (Join-Path $scratch 'hopC1')
    AEVAL_UPDATE_FEED   = "$feedUrl/v$VersionB/"
    AEVAL_UPDATE_EXPECT = 'current'
  }
  $null = $smoke.WaitForExit(180000)
  $text = App-Text 'hopC-current'
  Check 'hopC.version' ($text -match "\[SMOKE\] version=$([regex]::Escape($VersionB))") 'the updated app runs'
  Check 'hopC.current' ($text -match '\[SMOKE\] updateFeed=ok' -and $text -match '\[SMOKE\] update.status=current') "$(([regex]::Matches($text, '\[SMOKE\] update[^\r\n]*') | ForEach-Object Value) -join '; ')"
  Check 'hopC.smokeExit' ($smoke.ExitCode -eq 0) "code=$($smoke.ExitCode)"

  # Port 1 refuses; the machine is still online, so this is "the server did
  # not answer", which must read as a retry, not an error.
  $userD = Join-Path $scratch 'hopC2'
  $smoke = Start-App $exe 'hopC-unreachable' @{
    AEVAL_SMOKE         = '1'
    AEVAL_USERDATA      = $userD
    AEVAL_UPDATE_FEED   = 'http://127.0.0.1:1/'
    AEVAL_UPDATE_EXPECT = 'retrying,offline'
  }
  $null = $smoke.WaitForExit(180000)
  $text = App-Text 'hopC-unreachable'
  $updLines = ([regex]::Matches($text, '\[SMOKE\] update[^\r\n]*') | ForEach-Object Value) -join '; '
  Check 'hopC.retrying' ($text -match '\[SMOKE\] updateFeed=ok' -and $text -match '\[SMOKE\] updateQuiet=ok') $updLines
  $errLog = Get-Content (Join-Path $userD 'errors.log') -Raw -ErrorAction SilentlyContinue
  # The smoke test throws one deliberate rejection of its own; anything that
  # mentions the updater is the real thing.
  Check 'hopC.noUpdaterCrash' (-not ("$errLog" -match 'updater|electron-updater|ERR_CONNECTION')) 'a refused connection is not an unhandled error'
} finally {
  Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
  Get-Process AevalBuddy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  $entry = Find-Uninstall
  if ($entry) {
    $cmd = if ($entry.QuietUninstallString) { $entry.QuietUninstallString } else { "$($entry.UninstallString) /S" }
    $parts = [regex]::Match($cmd, '^"([^"]+)"\s*(.*)$')
    Start-Process -FilePath $parts.Groups[1].Value -ArgumentList $parts.Groups[2].Value -Wait
  }
}

# ------------------------------------------------------------------ report

$failed = @($checks | Where-Object { -not $_.Ok -and -not $_.Soft })
$warned = @($checks | Where-Object { -not $_.Ok -and $_.Soft })
$rows = $checks | ForEach-Object { "| $(if ($_.Ok) { 'ok' } elseif ($_.Soft) { 'warn' } else { '**FAIL**' }) | $($_.Name) | $($_.Detail.Replace('|', '\|')) |" }
$md = @(
  "## Update check: $prev -> $VersionA -> $VersionB"
  ''
  "$($checks.Count - $failed.Count - $warned.Count) of $($checks.Count) passed, $($warned.Count) soft warnings."
  ''
  '| | Check | Detail |'
  '| --- | --- | --- |'
) + $rows
$md -join "`n" | Set-Content (Join-Path $Out 'summary.md') -Encoding utf8
if ($env:GITHUB_STEP_SUMMARY) { $md -join "`n" | Add-Content $env:GITHUB_STEP_SUMMARY -Encoding utf8 }

foreach ($f in $failed) { Annotate 'error' $f.Name $f.Detail }
foreach ($w in $warned) { Annotate 'warning' $w.Name $w.Detail }
Annotate 'notice' "update check: $($checks.Count - $failed.Count) of $($checks.Count) passed" (($checks | ForEach-Object { "$($_.Level) $($_.Name) $($_.Detail)" }) -join "`n")

if ($failed.Count) { exit 1 }
