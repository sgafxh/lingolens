; Refresh shortcuts using the embedded application icon.
!macro NSIS_HOOK_POSTINSTALL
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
  ${EndIf}
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
