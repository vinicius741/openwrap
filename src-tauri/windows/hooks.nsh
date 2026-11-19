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

; OpenWrap needs OpenVPN Community. After installing OpenWrap, offer to install
; it with the bundled install-openvpn.ps1 (pinned, checksum-verified MSI) so a
; new user has nothing else to download. Skipped when OpenVPN is already
; installed, and in silent installs (/S), where the MessageBox answers No.
!macro NSIS_HOOK_POSTINSTALL
  Push $R7
  Push $R8
  Push $R9
  StrCpy $R8 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  StrCpy $R9 "$INSTDIR\install-openvpn.ps1"
  IfFileExists "$R9" 0 openwrap_openvpn_done
  nsExec::Exec '"$R8" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$R9" -CheckOnly'
  Pop $R7
  StrCmp $R7 "2" 0 openwrap_openvpn_done
  MessageBox MB_YESNO|MB_ICONQUESTION "OpenWrap needs OpenVPN Community (free) to connect, and it is not installed on this PC.$\r$\n$\r$\nDownload and install it now? It takes about a minute." /SD IDNO IDNO openwrap_openvpn_skipped
  DetailPrint "Installing OpenVPN Community..."
  nsExec::ExecToLog '"$R8" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$R9"'
  Pop $R7
  StrCmp $R7 "0" openwrap_openvpn_done
  MessageBox MB_OK|MB_ICONEXCLAMATION "OpenVPN could not be installed (see the details above).$\r$\n$\r$\nOpenWrap is installed. Open it and use Install OpenVPN, or install OpenVPN Community from openvpn.net." /SD IDOK
  Goto openwrap_openvpn_done
  openwrap_openvpn_skipped:
  DetailPrint "Skipped OpenVPN. OpenWrap offers to install it when it starts."
  openwrap_openvpn_done:
  Pop $R9
  Pop $R8
  Pop $R7
!macroend
