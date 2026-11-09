# Build a release of FluxCode on Windows and pack it like CI does:
#   powershell -ExecutionPolicy Bypass -File packaging/windows/build.ps1 [-Install]
# Output: dist\FluxCode-BuildDev-YYYY-MM-DD-HH-MM-SS-windows-x64.zip
# -Install also installs it for this user (install.ps1).
#
# CARGO_TARGET_DIR defaults to a path without spaces (MinGW windres breaks on them).
# HLSL shaders need fxc.exe: from the Windows SDK, or GPUI_FXC_PATH (tools/fxc-shim).
param([switch]$Install)
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = "$env:USERPROFILE\.fluxcode-target" }
if (-not $env:GPUI_FXC_PATH -and (Test-Path "$env:USERPROFILE\.fluxcode-tools\fxc.exe")) {
  $env:GPUI_FXC_PATH = "$env:USERPROFILE\.fluxcode-tools\fxc.exe"
}
# Same clock as CI: Asia/Jakarta (UTC+7).
$jakarta = [DateTime]::UtcNow.AddHours(7)
$env:FLUXCODE_BUILD = "BuildDev-" + $jakarta.ToString("yyyy-MM-dd-HH-mm-ss")
Write-Host "Building $env:FLUXCODE_BUILD"

Push-Location $root
try { cargo build --release; if ($LASTEXITCODE) { throw "cargo build failed" } } finally { Pop-Location }

$name = "FluxCode-$env:FLUXCODE_BUILD-windows-x64"
$stage = Join-Path $root "dist\$name"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$env:CARGO_TARGET_DIR\release\fluxcode.exe" "$stage\FluxCode.exe"
Copy-Item "$PSScriptRoot\install.ps1", "$PSScriptRoot\Install.cmd", "$PSScriptRoot\README.txt" $stage
$zip = Join-Path $root "dist\$name.zip"
Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "Packed $zip"

if ($Install) { & "$PSScriptRoot\install.ps1" -Exe "$stage\FluxCode.exe" }
