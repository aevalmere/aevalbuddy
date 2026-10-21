# Installs AevalBuddy-Setup.exe the way a person would, runs the smoke test and
# the visual self-check against the installed copy rather than the source tree,
# then uninstalls it and checks nothing was left behind.
#
# Meant for a throwaway Windows machine: .github/workflows/installer.yml runs
# it on a GitHub runner. On your own desktop it would install over your copy,
# so do not run it there.
#
#   ./tools/installer-check.ps1 -Installer dist\AevalBuddy-Setup.exe -Version 1.1.1 -Out out
#
# Results go to <Out>\: one log per app run, the screenshots, and summary.md.
# On a runner each failure is also an annotation, readable through the public
# API without signing in.

param(
  [Parameter(Mandatory)] [string] $Installer,
  [Parameter(Mandatory)] [string] $Version,
  [Parameter(Mandatory)] [string] $Out
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Installer = (Resolve-Path $Installer).Path
$onRunner = [bool]$env:GITHUB_ACTIONS
$scratch = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }

$checks = [System.Collections.Generic.List[object]]::new()
function Check([string]$name, [bool]$ok, [string]$detail = '') {
  $checks.Add([pscustomobject]@{ Name = $name; Ok = $ok; Detail = $detail })
  $mark = if ($ok) { 'ok  ' } else { 'FAIL' }
  Write-Host "[$mark] $name $detail"
}

# Workflow commands take one line, so newlines and % have to be encoded.
function Annotate([string]$level, [string]$title, [string]$message) {
  if (-not $onRunner) { return }
  $m = $message.Replace('%', '%25').Replace("`r", '%0D').Replace("`n", '%0A')
  $t = $title.Replace('%', '%25').Replace(':', '%3A').Replace(',', '%2C')
  Write-Host "::${level} title=${t}::$m"
}

# Without this an exception ends the run with an exit code and nothing else
# readable from outside the log.
trap {
  $at = $_.InvocationInfo
  Annotate 'error' 'installer-check.ps1' "line $($at.ScriptLineNumber): $($_.Exception.Message)`n$($at.Line.Trim())"
  exit 1
}

function Find-Uninstall {
  $keys = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*'
  Get-ItemProperty $keys -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'AevalBuddy*' } |
    Select-Object -First 1
}

# Starts the installed exe with extra environment, waits, and returns the exit
# code with its stdout. A fresh AEVAL_USERDATA per run gives each one its own
# settings and single-instance lock.
function Run-App([string]$exe, [string]$label, [hashtable]$vars, [int]$timeoutSec) {
  $log = Join-Path $Out "$label.log"
  $err = Join-Path $Out "$label.stderr.log"
  $saved = @{}
  foreach ($k in $vars.Keys) {
    $saved[$k] = [Environment]::GetEnvironmentVariable($k)
    [Environment]::SetEnvironmentVariable($k, $vars[$k])
  }
  try {
    $p = Start-Process -FilePath $exe -PassThru -RedirectStandardOutput $log -RedirectStandardError $err
    # ExitCode stays empty unless the handle is opened before the exit.
    $null = $p.Handle
    $code = if ($p.WaitForExit($timeoutSec * 1000)) { $p.ExitCode } else {
      Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
      'timeout'
    }
  } finally {
    foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
  }
  $text = (Get-Content $log -Raw -ErrorAction SilentlyContinue) + (Get-Content $err -Raw -ErrorAction SilentlyContinue)
  [pscustomobject]@{ Code = $code; Text = "$text" }
}

# ------------------------------------------------------------------ install

$p = Start-Process -FilePath $Installer -ArgumentList '/S' -PassThru -Wait
Check 'install.exitCode' ($p.ExitCode -eq 0) "code=$($p.ExitCode)"

$entry = Find-Uninstall
Check 'install.registered' ([bool]$entry) 'Add/Remove Programs entry'
if (-not $entry) {
  Annotate 'error' 'install' 'The installer exited without registering the app; nothing else can be checked.'
  exit 1
}
Check 'install.version' ($entry.DisplayVersion -eq $Version) "registered=$($entry.DisplayVersion) expected=$Version"

# electron-builder leaves InstallLocation out of this entry, so the folder
# comes from the uninstaller's path, which sits beside the exe.
$uninstaller = [regex]::Match($entry.UninstallString, '^"([^"]+)"').Groups[1].Value
$exe = Join-Path (Split-Path $uninstaller) 'AevalBuddy.exe'
Check 'install.exe' (Test-Path $exe) $exe
$fileVersion = (Get-Item $exe).VersionInfo.ProductVersion
Check 'install.exeVersion' ($fileVersion -eq $Version) "exe=$fileVersion"

$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\AevalBuddy.lnk'
$desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'AevalBuddy.lnk'
Check 'install.startMenuShortcut' (Test-Path $startMenu) $startMenu
Check 'install.desktopShortcut' (Test-Path $desktop) $desktop

# A silent install must not launch the app behind the person's back.
$running = @(Get-Process AevalBuddy -ErrorAction SilentlyContinue)
Check 'install.notLaunched' ($running.Count -eq 0) "processes=$($running.Count)"
$running | Stop-Process -Force -ErrorAction SilentlyContinue

# ------------------------------------------------------------------ run it

$smoke = Run-App $exe 'smoke' @{
  AEVAL_SMOKE    = '1'
  AEVAL_USERDATA = (Join-Path $scratch 'aeval-smoke')
} 180
$smokeLines = ($smoke.Text -split "`r?`n" | Where-Object { $_ -match '^\[SMOKE\]' }) -join "`n"
Check 'smoke.exitCode' ($smoke.Code -eq 0) "code=$($smoke.Code)"
Check 'smoke.printed' ([bool]$smokeLines) 'the packaged exe writes [SMOKE] lines to stdout'
foreach ($line in ($smokeLines -split "`n" | Where-Object { $_ -match '=FAIL' })) {
  Annotate 'error' 'smoke' $line
}
Annotate 'notice' 'smoke (installed exe)' $(if ($smokeLines) { $smokeLines } else { $smoke.Text })

$shotDir = Join-Path $Out 'shots'
New-Item -ItemType Directory -Force $shotDir | Out-Null
$shots = Run-App $exe 'shots' @{
  AEVAL_SHOT     = $shotDir
  AEVAL_USERDATA = (Join-Path $scratch 'aeval-shots')
} 300
$pngs = @(Get-ChildItem $shotDir -Filter *.png -ErrorAction SilentlyContinue)
Check 'shots.exitCode' ($shots.Code -eq 0) "code=$($shots.Code)"
Check 'shots.written' ($pngs.Count -gt 0) "png=$($pngs.Count)"
$shotLines = ($shots.Text -split "`r?`n" | Where-Object { $_ -match '^\[SHOT\]|^fit\.' }) -join "`n"
Annotate 'notice' "shots (installed exe): $($pngs.Count) png" $(if ($shotLines) { $shotLines } else { $shots.Text })

# ------------------------------------------------------------------ uninstall

$cmd = if ($entry.QuietUninstallString) { $entry.QuietUninstallString } else { "$($entry.UninstallString) /S" }
$parts = [regex]::Match($cmd, '^"([^"]+)"\s*(.*)$')
Start-Process -FilePath $parts.Groups[1].Value -ArgumentList $parts.Groups[2].Value -Wait
# The NSIS uninstaller copies itself to %TEMP% and returns at once, so the
# only reliable signal that it finished is the files going away.
$deadline = (Get-Date).AddSeconds(90)
while ((Test-Path $exe) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
Start-Sleep -Seconds 2
Check 'uninstall.exe' (-not (Test-Path $exe)) $exe
Check 'uninstall.registry' (-not (Find-Uninstall)) 'Add/Remove Programs entry'
Check 'uninstall.startMenuShortcut' (-not (Test-Path $startMenu)) ''
Check 'uninstall.desktopShortcut' (-not (Test-Path $desktop)) ''

# ------------------------------------------------------------------ report

$failed = @($checks | Where-Object { -not $_.Ok })
$rows = $checks | ForEach-Object { "| $(if ($_.Ok) { 'ok' } else { '**FAIL**' }) | $($_.Name) | $($_.Detail.Replace('|', '\|')) |" }
$md = @(
  "## Installer check: AevalBuddy $Version"
  ''
  "$($checks.Count - $failed.Count) of $($checks.Count) passed. The smoke test and the screenshots ran against the installed exe."
  ''
  '| | Check | Detail |'
  '| --- | --- | --- |'
) + $rows + @('', '```', $smokeLines, '```')
$md -join "`n" | Set-Content (Join-Path $Out 'summary.md') -Encoding utf8
if ($env:GITHUB_STEP_SUMMARY) { $md -join "`n" | Add-Content $env:GITHUB_STEP_SUMMARY -Encoding utf8 }

foreach ($f in $failed) { Annotate 'error' $f.Name $f.Detail }
Annotate 'notice' "installer check: $($checks.Count - $failed.Count) of $($checks.Count) passed" (($checks | ForEach-Object { "$(if ($_.Ok) { 'ok  ' } else { 'FAIL' }) $($_.Name) $($_.Detail)" }) -join "`n")

if ($failed.Count) { exit 1 }
