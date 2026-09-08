!macro NSIS_HOOK_PREUNINSTALL
  ; Always remove application-owned hooks before the installed executable is deleted.
  ; The built-in NSIS checkbox stores its state in $DeleteAppDataCheckboxState.
  ${If} $DeleteAppDataCheckboxState = 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-cleanup --purge' $0
  ${Else}
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-cleanup' $0
  ${EndIf}
  ${If} $0 <> 0
    MessageBox MB_ICONEXCLAMATION "Agent Mail Notifier could not finish uninstall cleanup. Some settings may require manual review."
  ${EndIf}
!macroend
