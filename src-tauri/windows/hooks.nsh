; Override the NSIS finish-page launch so "Run OpenWrap" works with OpenWrap's
; requireAdministrator manifest. Tauri's default RunMainBinary calls
; nsis_tauri_utils::RunAsUser, which fails with error 740 on elevated binaries.
; ExecShell delegates to ShellExecute, which triggers UAC when the installer ran
; unelevated and launches directly when already elevated.
!macro NSIS_HOOK_PREFINISHPAGE
  !undef MUI_FINISHPAGE_RUN_FUNCTION
  !define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinaryElevated

  Function RunMainBinaryElevated
    ExecShell "" "$INSTDIR\${MAINBINARYNAME}.exe"
  FunctionEnd
!macroend
