; NSIS hooks of the Corde installer (tauri.conf.json: bundle > windows > nsis > installerHooks).
; ASCII only: this file is !include'd by the Tauri NSIS template before its own !define lines,
; so ${PRODUCTNAME} & co are NOT available here - the product name is written out.
;
; Why this file exists
; --------------------
; In "currentUser" mode the Tauri installer proposes %LOCALAPPDATA%\<product name> as the
; install folder, i.e. %LOCALAPPDATA%\Corde - which is ALSO Corde's data folder (corde.db,
; sessions\, logs\, settings.json: corde_store::default_data_dir()). Program files and user
; data must not share a folder (uninstalling, wiping the data, measuring the installed size
; would all become ambiguous), so the default install folder becomes
; %LOCALAPPDATA%\Programs\Corde, the Windows convention for per-user installs.
;
; .onGUIInit runs after .onInit (which computed the default folder and restored the folder
; of a previous install from the registry) and before the first page is shown, so:
;   - first install            -> %LOCALAPPDATA%\Programs\Corde is proposed
;   - update / reinstall       -> the previous folder is kept
;   - /D=<folder> on the command line, or a folder picked on the Directory page -> respected
; Not covered: a fully silent FIRST install (/S), which has no GUI; it keeps Tauri's default
; (and later installs then stay there: an existing Corde.exe is never orphaned).

!include LogicLib.nsh

!define MUI_CUSTOMFUNCTION_GUIINIT CordeGuiInit

Function CordeGuiInit
  ${If} $INSTDIR == "$LOCALAPPDATA\Corde"
  ${AndIfNot} ${FileExists} "$LOCALAPPDATA\Corde\Corde.exe"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Corde"
  ${EndIf}
FunctionEnd
