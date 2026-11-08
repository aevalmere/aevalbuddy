# Fills %LOCALAPPDATA%\tauri\NSIS with the toolset `tauri build --bundles
# nsis` wants, so the bundler finds it complete and downloads nothing. Left
# to itself the bundler fetches NSIS and nsis_tauri_utils.dll from github.com
# release assets, and that endpoint has been answering 504 for whole days.
#
# The bundler keeps the folder only when every file in its required list is
# there, NSIS.exe has the SHA-1 of its NSIS release and the DLL has the SHA-1
# it pins; anything else makes it delete the folder and download again. The
# constants below are copied from crates/tauri-bundler/src/bundle/windows/
# nsis/mod.rs at the tag @tauri-apps/cli-v2.12.4 (NSIS_REQUIRED_FILES,
# NSIS_SHA1, NSIS_EXE_SHA1, NSIS_TAURI_UTILS_SHA1). The workflows build with
# exactly that CLI (TAURI_CLI in rust.yml and release.yml); move all three
# together, or the bundler goes back to downloading.
#
# Sources, first that works: the folder as it already is (actions/cache
# restored it), the preinstalled C:\Program Files (x86)\NSIS when it is the
# same release, then the official NSIS zip from SourceForge, then the copy on
# github.com, each checked against NSIS_SHA1. The DLL is vendored beside this
# script (tools/nsis/nsis_tauri_utils.dll, from the nsis-tauri-utils
# v0.5.3 release, MIT or Apache-2.0) and checked against the pinned SHA-1;
# the release URL is the fallback.
#
#   pwsh -NoProfile -File tools/seed-nsis.ps1 [-Dir <toolset folder>]
#
# Prints "[NSIS] ready ..." and exits 0, or says which source failed and
# exits 1.
param([string]$Dir = (Join-Path $env:LOCALAPPDATA 'tauri\NSIS'))

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Cli = '2.12.4'
$NsisVersion = '3.13'
$NsisZipSha1 = 'DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E'
$NsisExeSha1 = '6D45D4E8CCF18FE1FDAC9164058F356102A2ECF2'
$UtilsSha1 = '75197FEE3C6A814FE035788D1C34EAD39349B860'
$Utils = 'Plugins\x86-unicode\additional\nsis_tauri_utils.dll'
$Required = @(
  'makensis.exe',
  'Bin\makensis.exe',
  'Stubs\lzma-x86-unicode',
  'Stubs\lzma_solid-x86-unicode',
  $Utils,
  'Include\MUI2.nsh',
  'Include\FileFunc.nsh',
  'Include\x64.nsh',
  'Include\nsDialogs.nsh',
  'Include\WinMessages.nsh',
  'Include\Win\COM.nsh',
  'Include\Win\Propkey.nsh',
  'Include\Win\RestartManager.nsh'
)
$NsisUrls = @(
  "https://downloads.sourceforge.net/project/nsis/NSIS%203/$NsisVersion/nsis-$NsisVersion.zip",
  "https://sourceforge.net/projects/nsis/files/NSIS%203/$NsisVersion/nsis-$NsisVersion.zip/download",
  "https://github.com/tauri-apps/binary-releases/releases/download/nsis-$NsisVersion/nsis-$NsisVersion.zip"
)
$UtilsUrl = 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll'
$Preinstalled = 'C:\Program Files (x86)\NSIS'

if ($env:TAURI_CLI -and $env:TAURI_CLI -ne $Cli) {
  Write-Output "[NSIS] FAIL: the build runs tauri-cli $env:TAURI_CLI but these hashes are tauri-cli $Cli's; update tools/seed-nsis.ps1 from that tag's nsis/mod.rs"
  exit 1
}

function Sha1([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return '' }
  return (Get-FileHash -Algorithm SHA1 -LiteralPath $Path).Hash
}

function Missing([string]$Root) {
  return @($Required | Where-Object { -not (Test-Path -LiteralPath (Join-Path $Root $_)) })
}

# What the bundler checks before it decides to download.
function Problem([string]$Root) {
  if (-not (Test-Path -LiteralPath $Root)) { return 'no folder' }
  $gone = Missing $Root
  if ($gone.Count) { return "missing $($gone -join ', ')" }
  $exe = Sha1 (Join-Path $Root 'NSIS.exe')
  if ($exe -ne $NsisExeSha1) { return "NSIS.exe sha1 '$exe', want $NsisExeSha1" }
  $dll = Sha1 (Join-Path $Root $Utils)
  if ($dll -ne $UtilsSha1) { return "nsis_tauri_utils.dll sha1 '$dll', want $UtilsSha1" }
  return ''
}

function Fetch([string]$Url, [string]$Out, [string]$Want) {
  # curl's stderr is an answer here, not an error (Windows PowerShell would
  # throw on it under Stop).
  $ErrorActionPreference = 'Continue'
  for ($try = 1; $try -le 3; $try++) {
    Remove-Item -LiteralPath $Out -Force -ErrorAction SilentlyContinue
    $err = & curl.exe -sSL --fail --connect-timeout 20 --max-time 240 -o $Out $Url 2>&1
    $code = $LASTEXITCODE
    $got = Sha1 $Out
    if ($code -eq 0 -and $got -eq $Want) { return $true }
    $why = if ($code -ne 0) { "curl exit $code $err".Trim() } else { "sha1 $got, want $Want" }
    Write-Host "[NSIS] $Url try $try`: $why"
    if ($try -lt 3) { Start-Sleep -Seconds (5 * $try) }
  }
  return $false
}

$was = Problem $Dir
if (-not $was) {
  Write-Output "[NSIS] ready: $Dir already holds NSIS $NsisVersion for tauri-cli $Cli (from the cache)"
  exit 0
}
Write-Output "[NSIS] seeding $Dir ($was)"

# Staged beside the target, on the same volume, so the last step is a rename.
$parent = Split-Path -Parent $Dir
New-Item -ItemType Directory -Force $parent | Out-Null
$work = Join-Path $parent ".seed-$PID"
Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$stage = Join-Path $work 'NSIS'

try {
  $from = ''
  $preExe = Sha1 (Join-Path $Preinstalled 'NSIS.exe')
  if ($preExe -eq $NsisExeSha1) {
    Copy-Item -LiteralPath $Preinstalled -Destination $stage -Recurse
    $from = $Preinstalled
  } elseif ($preExe) {
    Write-Output "[NSIS] $Preinstalled is another NSIS release (NSIS.exe sha1 $preExe); the bundler would replace it"
  }
  if (-not $from) {
    $zip = Join-Path $work "nsis-$NsisVersion.zip"
    foreach ($u in $NsisUrls) {
      if (Fetch $u $zip $NsisZipSha1) { $from = $u; break }
    }
    if (-not $from) {
      Write-Output "[NSIS] FAIL: no source gave nsis-$NsisVersion.zip with sha1 $NsisZipSha1"
      exit 1
    }
    Expand-Archive -LiteralPath $zip -DestinationPath $work -Force
    Move-Item -LiteralPath (Join-Path $work "nsis-$NsisVersion") -Destination $stage
  }

  $dll = Join-Path $stage $Utils
  New-Item -ItemType Directory -Force (Split-Path -Parent $dll) | Out-Null
  $dllFrom = ''
  foreach ($c in @((Join-Path $Dir $Utils), (Join-Path $PSScriptRoot 'nsis\nsis_tauri_utils.dll'))) {
    if ((Sha1 $c) -eq $UtilsSha1) {
      Copy-Item -LiteralPath $c -Destination $dll -Force
      $dllFrom = $c
      break
    }
  }
  if (-not $dllFrom -and (Fetch $UtilsUrl $dll $UtilsSha1)) { $dllFrom = $UtilsUrl }
  if (-not $dllFrom) {
    Write-Output "[NSIS] FAIL: no source gave nsis_tauri_utils.dll with sha1 $UtilsSha1"
    exit 1
  }

  $bad = Problem $stage
  if ($bad) {
    Write-Output "[NSIS] FAIL: the staged toolset is not what the bundler wants: $bad"
    exit 1
  }
  Remove-Item -LiteralPath $Dir -Recurse -Force -ErrorAction SilentlyContinue
  Move-Item -LiteralPath $stage -Destination $Dir
} finally {
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

$bad = Problem $Dir
if ($bad) {
  Write-Output "[NSIS] FAIL: $Dir after seeding: $bad"
  exit 1
}
Write-Output "[NSIS] ready: NSIS $NsisVersion from $from, nsis_tauri_utils.dll from $dllFrom, in $Dir"
