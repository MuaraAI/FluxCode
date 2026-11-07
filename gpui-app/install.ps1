# Install the GPUI edition of FluxCode for the current user (no admin needed):
#   powershell -ExecutionPolicy Bypass -File gpui-app/install.ps1
# Copies the release exe, adds a Start Menu shortcut (so Windows Search finds
# "FluxCode") and an entry in Settings > Apps with a working Uninstall.
#
# The install folder is outside AppData on purpose: when this runs from a
# packaged (MSIX) host such as the Claude desktop app, writes to AppData\Local
# are redirected into that host's private package folder.
param(
  [string]$Exe = "$env:USERPROFILE\.fluxcode-target-gpui\release\fluxcode-gpui.exe",
  [string]$Dir = "$env:USERPROFILE\.fluxcode\app"
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "Build first: cargo build --release (missing $Exe)" }

# A running copy keeps the exe locked.
Get-Process FluxCode -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($Dir, "OrdinalIgnoreCase") } | Stop-Process -Force
New-Item -ItemType Directory -Force $Dir | Out-Null
Copy-Item $Exe "$Dir\FluxCode.exe" -Force
$version = (Get-Item "$Dir\FluxCode.exe").VersionInfo.ProductVersion

$shortcut = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\FluxCode.lnk"
$regKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FluxCode"

@"
# Uninstall FluxCode (GPUI edition). Settings and session in %APPDATA%\FluxCode-GPUI are kept.
Get-Process FluxCode -ErrorAction SilentlyContinue | Where-Object { `$_.Path -eq '$Dir\FluxCode.exe' } | Stop-Process -Force
Remove-Item '$shortcut' -Force -ErrorAction SilentlyContinue
Remove-Item '$regKey' -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300
Remove-Item '$Dir' -Recurse -Force -ErrorAction SilentlyContinue
"@ | Set-Content -Encoding utf8 "$Dir\uninstall.ps1"

$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut($shortcut)
$lnk.TargetPath = "$Dir\FluxCode.exe"
$lnk.WorkingDirectory = $env:USERPROFILE
$lnk.IconLocation = "$Dir\FluxCode.exe,0"
$lnk.Description = "FluxCode - lightweight code editor (native GPUI edition)"
$lnk.Save()

New-Item -Path $regKey -Force | Out-Null
$size = [int]((Get-Item "$Dir\FluxCode.exe").Length / 1KB)
$props = @{
  DisplayName = "FluxCode"; DisplayVersion = $version; Publisher = "MuaraAI"
  DisplayIcon = "$Dir\FluxCode.exe,0"; InstallLocation = $Dir
  UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$Dir\uninstall.ps1`""
}
foreach ($k in $props.Keys) { Set-ItemProperty -Path $regKey -Name $k -Value $props[$k] }
Set-ItemProperty -Path $regKey -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -Path $regKey -Name NoRepair -Value 1 -Type DWord
Set-ItemProperty -Path $regKey -Name EstimatedSize -Value $size -Type DWord

Write-Host "Installed FluxCode $version to $Dir"
Write-Host "Start Menu: $shortcut"
