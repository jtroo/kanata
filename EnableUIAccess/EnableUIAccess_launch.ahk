#requires AutoHotkey v2.0
#SingleInstance Off ; Needed for elevation with *runas.
/* v2 based on EnableUIAccess.ahk v1.01 by Lexikos USE AT YOUR OWN RISK
  Enables the uiAccess flag in an application's embedded manifest and signs the file with a self-signed digital certificate. If the file is in a trusted location (A_ProgramFiles or A_WinDir), this allows the application to bypass UIPI (User Interface Privilege Isolation, a part of User Account Control in Vista/7). It also enables the journal playback hook (SendPlay).
  Command line params (mutually exclusive):
    SkipWarning     - don't display the initial warning
    "<in>" "<out>"  - attempt to run silently using the given file(s)
  This script and the provided Lib files may be used, modified, copied, etc. without restriction.
*/
#include <EnableUIAccess>

in_file  := (A_Args.Has(1))?A_Args[1]:'' ; Command line args
out_file := (A_Args.Has(2))?A_Args[2]:''

if (in_file = ""){
  msgResult := MsgBox("Enable the selected EXE to bypass UAC-UIPI security restrictions imposed by modifying 'UIAccess' attribute in the file's embedded manifest and signing the file using a self-signed digital certificate, which is then installed in the local machine's Trusted Root Certification Authorities store.`n`nThe resulting EXE is unusable on a system without this certificate installed!`n`nContinue at your own risk", "", 49)
  if (msgResult = "Cancel"){
    ExitApp()
  }
}

if !A_IsAdmin {
  if (in_file = "") {
    in_file := "SkipWarning"
  }
  cmd := "`"" . A_ScriptFullPath . "`""
  if !A_IsCompiled {   ; Use A_AhkPath in case the "runas" verb isn't registered for ahk files.
    cmd := "`"" . A_AhkPath . "`" " . cmd
  }
  Try Run("*RunAs " cmd " `"" in_file "`" `"" out_file "`"", , "", )
  ExitApp()
}
global user_specified_files := false
if (in_file = "" || in_file = "SkipWarning") { ; Find AutoHotkey installation.
  InstallDir := RegRead("HKEY_LOCAL_MACHINE\SOFTWARE\AutoHotkey", "InstallDir")
  if A_LastError && A_PtrSize=8 {
    InstallDir := RegRead("HKLM\SOFTWARE\Wow6432Node\AutoHotkey", "InstallDir")
  }
  ; Let user confirm or select file(s).
  in_file := FileSelect(1, InstallDir "\AutoHotkey.exe", "Select Source File", "Executable Files (*.exe)")
  if A_LastError {
    ExitApp()
  }
  out_file := FileSelect("S16", in_file, "Select Destination File", "Executable Files (*.exe)")
  if A_LastError {
    ExitApp()
  }
  user_specified_files := true
}

Loop in_file { ; Convert short paths to long paths
  in_file := A_LoopFileFullPath
}
if (out_file = "") {   ; i.e. only one file was given via command line
  out_file := in_file
} else {
  Loop out_file {
    out_file := A_LoopFileFullPath
  }
}
if Crypt.IsSigned(in_file) {
  msgResult := MsgBox("Input file is already signed.  The script will now exit" in_file,"", 48)
  ExitApp()
}

if user_specified_files && !IsTrustedLocation(out_file) {
  msgResult := MsgBox("Target path is not a trusted location (Program Files or Windows\System32), so 'uiAccess'  will have no effect until the file is moved there","", 49)
  if (msgResult = "Cancel") {
    ExitApp()
  }
}

if (in_file = out_file) { ; The following should typically work even if the file is in use
  bak_file := in_file "~" A_Now ".bak"
  FileMove(in_file, bak_file, 1)
  if A_LastError {
    Fail("Failed to rename selected file.")
  }
  in_file := bak_file
}
Try {
  FileCopy(in_file, out_file, 1)
} Catch as Err {
  throw OSError(Err)
}
if A_LastError {
  Fail("Failed to copy file to destination.")
}

if !EnableUIAccess(out_file) { ; Set the uiAccess attribute in the file's manifest
  Fail("Failed to set uiAccess attribute in manifest")
}


if (user_specified_files && in_file != out_file) { ; in interactive mode, if not overwriting the original file, offer to create an additional context menu item for AHK files
  uiAccessVerb := RegRead("HKCR\AutoHotkeyScript\Shell\uiAccess\Command")
  if A_LastError {
    msgResult := MsgBox("Register `"Run Script with UI Access`" context menu item?", "", 3)
    if (msgResult = "Yes") {
      RegWrite("Run with UI Access", "REG_SZ", "HKCR\AutoHotkeyScript\Shell\uiAccess")
      RegWrite("`"" out_file "`" `"`%1`" `%*", "REG_SZ", "HKCR\AutoHotkeyScript\Shell\uiAccess\Command")
    }
    if (msgResult = "Cancel")
      ExitApp()
  }
}

IsTrustedLocation(path) { ; IsTrustedLocation  →true if path is a valid location for uiAccess="true"
  ; http://msdn.microsoft.com/en-us/library/bb756929 "\Program Files\ and \windows\system32\ are currently 2 allowable protected locations." However, \Program Files (x86)\ also appears to be allowed
  if InStr(path, A_ProgramFiles "\") = 1 {
    return true
  }
  if InStr(path, A_WinDir "\System32\") = 1 {
    return true
  }
  other := EnvGet(A_PtrSize=8 ? "ProgramFiles(x86)" : "ProgramW6432") ; On 64-bit systems, if this script is 32-bit, A_ProgramFiles is %ProgramFiles(x86)%, otherwise it is %ProgramW6432%. So check the opposite "Program Files" folder:
  if (other != "" && InStr(path, other "\") = 1) {
    return true
  }
  return   false
}

Fail(msg) {
  ; if (%True% != "Silent") { ;???
    MsgBox(msg "`nA_LastError: " A_LastError, "", 16)
  ; }
  ExitApp()
}

Warn(msg) {
  msg .= " (Err " A_LastError ")`n"
  OutputDebug(msg)
  FileAppend(msg, "*")
}
