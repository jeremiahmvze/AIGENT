; AIGENT for Windows installer. Per-user (no admin prompt), like dragging the Mac app to Applications.
; Built by build.sh: makensis -DVERSION=x.y installer.nsi
Unicode true
!include MUI2.nsh

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\AIGENT"

Name "AIGENT"
OutFile "..\build\AIGENT-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\AIGENT"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "AIGENT"
VIAddVersionKey "FileDescription" "AIGENT Setup"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT licensed"

!define MUI_ICON "assets\aigent.ico"
!define MUI_UNICON "assets\aigent.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\AIGENT.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open AIGENT"
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!define MUI_UNCONFIRMPAGE_TEXT_TOP "Your profiles' sign-ins and data are kept. Profile shortcuts in the Start Menu are removed."
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Profile watchers (AIGENT.exe --launch) keep the exe open. Ending them only stops taskbar tagging for
; windows opened later; running profiles keep going.
!macro StopAIGENT
  nsExec::Exec 'taskkill /IM AIGENT.exe /F'
  Pop $0
  Sleep 500
!macroend

Section "AIGENT"
  !insertmacro StopAIGENT
  SetOutPath "$INSTDIR"
  File "..\build\windows\AIGENT.exe"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\Uninstall AIGENT.exe"
  CreateShortcut "$SMPROGRAMS\AIGENT.lnk" "$INSTDIR\AIGENT.exe"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "AIGENT"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "AIGENT"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\AIGENT.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall AIGENT.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall AIGENT.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" 7000
SectionEnd

Section "Uninstall"
  !insertmacro StopAIGENT
  Delete "$INSTDIR\AIGENT.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\Uninstall AIGENT.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\AIGENT.lnk"
  ; Profile shortcuts start AIGENT.exe, so they'd be broken without it. Reinstalling recreates them.
  RMDir /r "$SMPROGRAMS\AI Profiles"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "AIGENT"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  ; Kept: %APPDATA%\AIGENT (profile list, favorites) and %LOCALAPPDATA%\AIGENT (profile data, icons).
SectionEnd
