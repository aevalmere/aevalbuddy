; Tauri NSIS hooks (ARCHITECTURE.md D6, PARITY UPD-15).
;
; The app was AevalBuddy before 2.0; this installer replaces a copy under
; that name.
;
; PREINSTALL: an Electron copy installed by electron-builder (per user, under
; %LOCALAPPDATA%\Programs\aevalbuddy) is uninstalled silently first. Its
; uninstaller keeps %APPDATA%\aevalbuddy (deleteAppDataOnUninstall: false);
; the app moves that folder to %APPDATA%\aevaleron on its first start
; (paths.rs), so the settings, key, history and shortcuts carry over. The Run
; key is rewritten by the new app at boot (autostart.rs).
;
; A 2.0 test build from before the rename (Tauri, in %LOCALAPPDATA%\AevalBuddy
; with uninstall.exe beside AevalBuddy.exe) is removed the same way, with the
; browser profile and update downloads it kept in its install folder.
;
; electron-builder's uninstall entry is "AevalBuddy <version>", and it does
; not always write InstallLocation, so the entry is matched by the first ten
; characters of DisplayName and the folder is taken from UninstallString
; ("<dir>\Uninstall AevalBuddy.exe" /currentuser), falling back to
; InstallLocation.
;
; POSTINSTALL: electron-updater and our own updater (updater.rs) start the
; installer with `/S --updated [--force-run]`; `--force-run` relaunches the
; app once the files are in place.

!include "FileFunc.nsh"

!macro NSIS_HOOK_PREINSTALL
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  StrCpy $0 0
  aeval_scan:
    EnumRegKey $1 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" aeval_done
    IntOp $0 $0 + 1
    ReadRegStr $2 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    StrCpy $4 $2 10
    StrCmp $4 "AevalBuddy" 0 aeval_scan
    ReadRegStr $3 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$1" "UninstallString"
    StrCmp $3 "" aeval_location
    ; A quoted path: keep what is between the quotes.
    StrCpy $4 $3 1
    StrCmp $4 '"' 0 aeval_parent
    StrCpy $3 $3 "" 1
    StrCpy $5 0
  aeval_quote:
    StrCpy $4 $3 1 $5
    StrCmp $4 "" aeval_cut
    StrCmp $4 '"' aeval_cut
    IntOp $5 $5 + 1
    Goto aeval_quote
  aeval_cut:
    StrCpy $3 $3 $5
  aeval_parent:
    ${GetParent} "$3" $3
    StrCmp $3 "" aeval_location aeval_check
  aeval_location:
    ReadRegStr $3 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$1" "InstallLocation"
    StrCmp $3 "" aeval_scan
  aeval_check:
    ; electron-builder names its uninstaller "Uninstall <productName>.exe";
    ; Tauri's own is uninstall.exe.
    IfFileExists "$3\Uninstall AevalBuddy.exe" aeval_electron
    IfFileExists "$3\AevalBuddy.exe" 0 aeval_scan
    IfFileExists "$3\uninstall.exe" 0 aeval_scan
    DetailPrint "Removing the previous AevalBuddy from $3"
    ExecWait '"$3\uninstall.exe" /S _?=$3'
    Delete "$3\uninstall.exe"
    RMDir /r "$3\WebView2"
    RMDir /r "$3\pending"
    RMDir "$3"
    Goto aeval_done
  aeval_electron:
    DetailPrint "Removing the previous AevalBuddy (Electron) from $3"
    ; _?= runs the uninstaller in place and waits for it.
    ExecWait '"$3\Uninstall AevalBuddy.exe" /S /currentuser --updated _?=$3'
    Delete "$3\Uninstall AevalBuddy.exe"
    RMDir "$3"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$1"
  aeval_done:
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "--force-run" $0
  IfErrors aeval_norun
    Exec '"$INSTDIR\Aevaleron.exe"'
  aeval_norun:
  Pop $0
!macroend
