; Fresh Shell Links use the standalone icon with an absolute path.
!macro LingoLensShortcut path
  ${If} ${FileExists} "${path}"
    !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 ""
    ${If} $0 P<> 0
      ${IShellLink::SetPath} $0 '(w "$INSTDIR\lingolens.exe")'
      ${IShellLink::SetWorkingDirectory} $0 '(w "$INSTDIR")'
      ${IShellLink::SetDescription} $0 '(w "LingoLens")'
      ${IShellLink::SetIconLocation} $0 '(w "$INSTDIR\icons\LingoLens.ico", i 0)'
      ${IUnknown::QueryInterface} $0 '("${IID_IShellLinkDataList}",.r2)'
      ${If} $2 P<> 0
        ${IShellLinkDataList::RemoveDataBlock} $2 '(i 0xA0000007)'
        ${IShellLinkDataList::GetFlags} $2 '(*i.r3)'
        IntOp $3 $3 & 0xFFFFBFFF
        ${IShellLinkDataList::SetFlags} $2 '(i $3)'
        ${IUnknown::Release} $2 ""
      ${EndIf}
      ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}",.r1)'
      ${If} $1 P<> 0
        ${IPersistFile::Save} $1 '(w "${path}",i 1)i.r3'
        ${If} $3 < 0
          DetailPrint "Could not update icon for ${path}: $3"
        ${EndIf}
        ${IUnknown::Release} $1 ""
      ${EndIf}
      ${IUnknown::Release} $0 ""
    ${EndIf}
    System::Call 'shell32::SHChangeNotify(i 0x2000, i 0x1005, w "${path}", p 0)'
  ${EndIf}
!macroend

Function LingoLensRefreshIcons
  ${IfNot} ${FileExists} "$INSTDIR\icons\LingoLens.ico"
    Return
  ${EndIf}
  Push $0
  Push $1
  Push $2
  Push $3
  !insertmacro LingoLensShortcut "$DESKTOP\LingoLens.lnk"
  !insertmacro LingoLensShortcut "$SMPROGRAMS\LingoLens.lnk"
  System::Call 'shell32::SHChangeNotify(i 0x1000, i 0x1005, w "$DESKTOP", p 0)'
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  Call LingoLensRefreshIcons
!macroend

; Interactive desktop shortcuts are created after POSTINSTALL.
Function .onGUIEnd
  Call LingoLensRefreshIcons
FunctionEnd
