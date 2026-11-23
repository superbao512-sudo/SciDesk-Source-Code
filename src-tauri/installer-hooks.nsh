; The folder is runtime-owned, deliberately absent from the bundle resource list.
; Normal uninstall leaves it intact, including all personal data.
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$INSTDIR\SciDeskData"
!macroend
