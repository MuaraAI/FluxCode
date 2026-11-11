; FluxCode's Windows installer: one .exe that installs for the current user (no admin).
;   makensis /DBUILD=BuildDev-2026-11-10-21-00-00 /DEXE=path\FluxCode.exe /DOUT=dist\FluxCode-Setup.exe packaging\windows\installer.nsi
;
; It does what install.ps1 does, so either can install or replace the other: the app in
; %USERPROFILE%\.fluxcode\app (where FluxCode updates itself), a Start Menu shortcut (so
; Windows Search finds "FluxCode"), a Desktop shortcut, and an entry in Settings > Apps
; with a working Uninstall. Settings and the session in %APPDATA%\FluxCode-GPUI are kept.
;
; The folder is outside AppData on purpose: from a packaged (MSIX) host, writes to
; AppData\Local are redirected into that host's private folder.

!ifndef BUILD
  !error "pass /DBUILD=BuildDev-..."
!endif
!ifndef EXE
  !error "pass /DEXE=path\to\FluxCode.exe"
!endif
!ifndef OUT
  !define OUT "FluxCode-${BUILD}-windows-x64-setup.exe"
!endif

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma
; The whole exe in one window: LZMA finds more repeats than with the default 8 MB.
SetCompressorDictSize 64

!define APP "FluxCode Development"
!define REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\FluxCode"

Name "${APP}"
Caption "${APP} setup"
BrandingText "${BUILD}"
OutFile "${OUT}"
InstallDir "$PROFILE\.fluxcode\app"

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ICON "..\..\assets\icon.ico"
!define MUI_UNICON "..\..\assets\icon.ico"
!define MUI_ABORTWARNING

!define MUI_WELCOMEPAGE_TITLE "Install ${APP}"
!define MUI_WELCOMEPAGE_TEXT "A native code editor that runs as one process.$\r$\n$\r$\nThis installs build ${BUILD} for your Windows account only. No administrator rights are needed.$\r$\n$\r$\nFluxCode goes in $INSTDIR and updates itself there. Your settings are kept."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\FluxCode.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open FluxCode"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; A running FluxCode from this folder keeps the exe locked.
!macro StopRunning
  nsExec::Exec `powershell.exe -NoProfile -NonInteractive -Command "Get-Process FluxCode -ErrorAction SilentlyContinue | Where-Object { $$_.Path -and $$_.Path.StartsWith('$INSTDIR', 'OrdinalIgnoreCase') } | Stop-Process -Force"`
  Pop $0
  Sleep 300
!macroend

Section "FluxCode"
  !insertmacro StopRunning
  SetOutPath "$INSTDIR"
  File "/oname=FluxCode.exe" "${EXE}"
  ; What the last self-update left behind.
  Delete "$INSTDIR\FluxCode.exe.old"
  Delete "$INSTDIR\FluxCode.exe.new"
  ; install.ps1's uninstaller, replaced by this one.
  Delete "$INSTDIR\uninstall.ps1"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  SetOutPath "$PROFILE"
  CreateShortcut "$SMPROGRAMS\FluxCode.lnk" "$INSTDIR\FluxCode.exe" "" "$INSTDIR\FluxCode.exe" 0 SW_SHOWNORMAL "" "FluxCode Development - lightweight native code editor"
  CreateShortcut "$DESKTOP\FluxCode.lnk" "$INSTDIR\FluxCode.exe" "" "$INSTDIR\FluxCode.exe" 0 SW_SHOWNORMAL "" "FluxCode Development - lightweight native code editor"

  WriteRegStr HKCU "${REGKEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${REGKEY}" "DisplayVersion" "${BUILD}"
  WriteRegStr HKCU "${REGKEY}" "Publisher" "MuaraAI"
  WriteRegStr HKCU "${REGKEY}" "DisplayIcon" "$INSTDIR\FluxCode.exe,0"
  WriteRegStr HKCU "${REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${REGKEY}" "URLInfoAbout" "https://github.com/MuaraAI/FluxCode"
  WriteRegStr HKCU "${REGKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${REGKEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${REGKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${REGKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${REGKEY}" "EstimatedSize" "$0"

  ; The shell caches a shortcut's icon by path: a reinstall with a new icon kept the
  ; old one on the taskbar and in Start until this.
  nsExec::Exec '"$SYSDIR\ie4uinit.exe" -show'
  Pop $0
SectionEnd

Section "Uninstall"
  !insertmacro StopRunning
  Delete "$SMPROGRAMS\FluxCode.lnk"
  Delete "$DESKTOP\FluxCode.lnk"
  Delete "$INSTDIR\FluxCode.exe"
  Delete "$INSTDIR\FluxCode.exe.old"
  Delete "$INSTDIR\FluxCode.exe.new"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${REGKEY}"
SectionEnd
