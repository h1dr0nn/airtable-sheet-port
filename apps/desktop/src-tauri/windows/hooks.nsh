; Tauri NSIS installer hooks (bundle.windows.nsis.installerHooks).
;
; MCP clients (Claude Desktop/Code, ...) spawn sheet-port-mcp.exe from the
; install dir and keep it running, so an update cannot overwrite it. Windows
; does allow renaming a running exe on the same volume, so before the files
; are copied the old sidecar is moved aside to sheet-port-mcp.old-N.exe and the
; new one is written under the original name. Running clients keep the old
; process until they restart it. Leftover .old-N.exe files are deleted on the
; next install, on app startup (lib.rs) and on uninstall.
;
; Loops use LogicLib, which generates unique labels, so no hand-written labels
; can clash with the Tauri template.

!include LogicLib.nsh

!define SP_SIDECAR "sheet-port-mcp"

!macro NSIS_HOOK_PREINSTALL
  Push $0
  Push $1

  ; 1. Remove leftovers from earlier updates. Copies still running fail to
  ;    delete, which is fine: they are retried next time.
  FindFirst $0 $1 "$INSTDIR\${SP_SIDECAR}.old-*.exe"
  ${DoWhile} $1 != ""
    Delete "$INSTDIR\$1"
    FindNext $0 $1
  ${Loop}
  FindClose $0

  ; 2. Move the current sidecar aside to the first free .old-N.exe name.
  ${If} ${FileExists} "$INSTDIR\${SP_SIDECAR}.exe"
    StrCpy $0 1
    ${DoWhile} ${FileExists} "$INSTDIR\${SP_SIDECAR}.old-$0.exe"
      IntOp $0 $0 + 1
    ${Loop}
    ClearErrors
    Rename "$INSTDIR\${SP_SIDECAR}.exe" "$INSTDIR\${SP_SIDECAR}.old-$0.exe"
    ${If} ${Errors}
      DetailPrint "Could not move ${SP_SIDECAR}.exe aside; it may stay at the old version until the MCP client using it is closed."
    ${Else}
      DetailPrint "Moved ${SP_SIDECAR}.exe to ${SP_SIDECAR}.old-$0.exe"
    ${EndIf}
  ${EndIf}

  Pop $1
  Pop $0
  ; 3. The installer now writes the new sidecar under the original name.
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Old copies still in use are deleted at the next reboot. The uninstaller
  ; also runs as part of an update (/UPDATE); then the new installer cleans
  ; up instead, so skip /REBOOTOK there and never ask for a reboot.
  Push $0
  Push $1
  FindFirst $0 $1 "$INSTDIR\${SP_SIDECAR}.old-*.exe"
  ${DoWhile} $1 != ""
    ${If} $UpdateMode = 1
      Delete "$INSTDIR\$1"
    ${Else}
      Delete /REBOOTOK "$INSTDIR\$1"
    ${EndIf}
    FindNext $0 $1
  ${Loop}
  FindClose $0
  ; No /REBOOTOK for ${SP_SIDECAR}.exe itself: a reinstall before the reboot
  ; would put the new sidecar at that path and the pending delete would take it.
  Pop $1
  Pop $0
!macroend
