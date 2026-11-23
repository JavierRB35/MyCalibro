Unicode true
RequestExecutionLevel user
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
Name "MyCalibro"
OutFile "..\dist\MyCalibro-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\MyCalibro"
ShowInstDetails show
ShowUninstDetails show
Var TestMode

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Spanish"

Function .onInit
  SetShellVarContext current
  StrCpy $TestMode "0"
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/TESTMODE" $R1
  IfErrors normal
  StrCpy $TestMode "1"
  normal:
FunctionEnd

Section "MyCalibro"
  SetOutPath "$INSTDIR"
  File /oname=MyCalibro.exe "..\app\target\release\biblioteca_sencilla.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  StrCmp $TestMode "1" test_install
  CreateDirectory "$SMPROGRAMS\MyCalibro"
  CreateShortcut "$SMPROGRAMS\MyCalibro\MyCalibro.lnk" "$INSTDIR\MyCalibro.exe"
  CreateShortcut "$SMPROGRAMS\MyCalibro\Desinstalar.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro" "DisplayName" "MyCalibro"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro" "NoRepair" 1
  Goto done
  test_install:
  FileOpen $R0 "$INSTDIR\.test-install" w
  FileClose $R0
  done:
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  IfFileExists "$INSTDIR\.test-install" skip_registration
  Delete "$SMPROGRAMS\MyCalibro\MyCalibro.lnk"
  Delete "$SMPROGRAMS\MyCalibro\Desinstalar.lnk"
  RMDir "$SMPROGRAMS\MyCalibro"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyCalibro"
  skip_registration:
  ; Never remove user libraries, settings, backups, or arbitrary files recursively.
  Delete "$INSTDIR\MyCalibro.exe"
  Delete "$INSTDIR\.test-install"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
