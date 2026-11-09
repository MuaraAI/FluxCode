@echo off
rem Install FluxCode for this user: no admin, Start Menu shortcut, entry in Settings > Apps.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" -Exe "%~dp0FluxCode.exe"
if errorlevel 1 (
  echo.
  echo The install did not finish. The error is above.
  pause
  exit /b 1
)
echo.
echo Done. Open FluxCode from the Start Menu.
timeout /t 4 >nul
