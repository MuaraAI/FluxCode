# Install FluxCode for the current user (no admin needed).
#   From the release ZIP: double-click Install.cmd (it runs this script).
#   From a build:         powershell -ExecutionPolicy Bypass -File packaging/windows/install.ps1 -Exe <path>\fluxcode.exe
# Copies the exe, adds a Start Menu shortcut (so Windows Search finds "FluxCode")
# and an entry in Settings > Apps with a working Uninstall.
#
# The install folder is outside AppData on purpose: when this runs from a
# packaged (MSIX) host, writes to AppData\Local are redirected into that host's
# private package folder. FluxCode updates itself in this folder.
param(
  [string]$Exe = "$PSScriptRoot\FluxCode.exe",
  [string]$Dir = "$env:USERPROFILE\.fluxcode\app"
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "FluxCode.exe not found at $Exe" }

# A running copy keeps the exe locked.
Get-Process FluxCode -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($Dir, "OrdinalIgnoreCase") } | Stop-Process -Force
New-Item -ItemType Directory -Force $Dir | Out-Null
Copy-Item $Exe "$Dir\FluxCode.exe" -Force
# The build ID (BuildDev-YYYY-MM-DD-HH-MM-SS) is the exe's product version.
$build = (Get-Item "$Dir\FluxCode.exe").VersionInfo.ProductVersion

$shortcut = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\FluxCode.lnk"
$regKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FluxCode"

@"
# Uninstall FluxCode. Settings and the session in %APPDATA%\FluxCode-GPUI are kept.
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
$lnk.Description = "FluxCode Development - lightweight native code editor"
$lnk.Save()

New-Item -Path $regKey -Force | Out-Null
$size = [int]((Get-Item "$Dir\FluxCode.exe").Length / 1KB)
$props = @{
  DisplayName = "FluxCode Development"; DisplayVersion = $build; Publisher = "MuaraAI"
  DisplayIcon = "$Dir\FluxCode.exe,0"; InstallLocation = $Dir
  URLInfoAbout = "https://github.com/MuaraAI/FluxCode"
  UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$Dir\uninstall.ps1`""
}
foreach ($k in $props.Keys) { Set-ItemProperty -Path $regKey -Name $k -Value $props[$k] }
Set-ItemProperty -Path $regKey -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -Path $regKey -Name NoRepair -Value 1 -Type DWord
Set-ItemProperty -Path $regKey -Name EstimatedSize -Value $size -Type DWord

Write-Host "Installed FluxCode $build to $Dir"
Write-Host "Start Menu: $shortcut"
