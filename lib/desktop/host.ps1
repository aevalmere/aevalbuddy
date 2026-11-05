# Boots the desktop helper. lib/desktop/index.js does not run this file: it
# reads it (Electron can read inside app.asar, powershell.exe cannot) and
# sends it over stdin as one line, after setting three variables:
#
#   $AevalCs       the C# source of host.cs
#   $AevalCache    folder for the compiled assembly ('' to skip the cache)
#   $AevalVersion  the app version, echoed in the ready event
#
# Compiling host.cs takes one to two seconds, so the assembly is kept in
# %LOCALAPPDATA%\aevalbuddy\desktop-<hash>.dll, keyed by a hash of the source,
# and later sessions load it in a few milliseconds. A source change is a new
# hash and so a new file; nothing goes stale.

$ErrorActionPreference = 'Stop'
try {
  Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, WindowsBase, System.Web.Extensions, System.Drawing
  $wrt = [Reflection.Assembly]::Load('System.Runtime.WindowsRuntime, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089')
  $refs = @(
    [System.Windows.Automation.AutomationElement].Assembly.Location
    [System.Windows.Automation.ControlType].Assembly.Location
    [System.Windows.Rect].Assembly.Location
    [System.Web.Script.Serialization.JavaScriptSerializer].Assembly.Location
    [System.Drawing.Bitmap].Assembly.Location
    $wrt.Location
    'System.Runtime'
  )
  $sha = [Security.Cryptography.SHA256]::Create()
  $hash = -join ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($AevalCs))[0..7] | ForEach-Object { $_.ToString('x2') })
  $loaded = $false
  if ($AevalCache) {
    $dll = Join-Path $AevalCache "desktop-$hash.dll"
    if (Test-Path $dll) {
      try { Add-Type -Path $dll; $loaded = $true } catch { $loaded = $false }
    }
    if (-not $loaded) {
      try {
        New-Item -ItemType Directory -Force $AevalCache | Out-Null
        # Another helper may be compiling the same source; each writes its
        # own file and the rename settles it.
        $tmp = Join-Path $AevalCache "desktop-$hash.$PID.tmp.dll"
        Add-Type -TypeDefinition $AevalCs -ReferencedAssemblies $refs -OutputAssembly $tmp -OutputType Library
        try { Move-Item -LiteralPath $tmp -Destination $dll -Force } catch { $dll = $tmp }
        Add-Type -Path $dll
        $loaded = $true
      } catch { $loaded = $false }
    }
  }
  if (-not $loaded) { Add-Type -TypeDefinition $AevalCs -ReferencedAssemblies $refs }
} catch {
  $msg = @{ event = 'fatal'; error = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
  [Console]::Out.WriteLine($msg)
  exit 2
}
[AevalDesktop.Host]::Run($AevalVersion)
exit 0
