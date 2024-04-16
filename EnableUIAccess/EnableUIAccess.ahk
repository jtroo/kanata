/*
    EnableUIAccess.ahk v1.01 by Lexikos
    
    USE AT YOUR OWN RISK

    Enables the uiAccess flag in an application's embedded manifest
    and signs the file with a self-signed digital certificate.  If the
    file is in a trusted location (A_ProgramFiles or A_WinDir), this
    allows the application to bypass UIPI (User Interface Privilege
    Isolation, a part of User Account Control in Vista/7).  It also
    enables the journal playback hook (SendPlay).
    
    Command line params (mutually exclusive):
        SkipWarning     - don't display the initial warning
        "<in>" "<out>"  - attempt to run silently using the given file(s)

    This script and the provided Lib files may be used, modified,
    copied, etc. without restriction.

*/

#NoEnv

#Include <Cert>
#Include <Crypt>
#Include <SignFile>
#Include <SystemTime>

; Command line args:
in_file = %1%
out_file = %2%

if (in_file = "")
MsgBox 49,,
(Join
This script enables the selected AutoHotkey.exe to bypass restrictions
 imposed by UIPI, a component of UAC in Windows Vista and 7.  To do this
 it modifies an attribute in the file's embedded manifest and signs the
 file using a self-signed digital certificate, which is then installed
 in the local machine's Trusted Root Certification Authorities store.`n
`n
THE RESULTING EXECUTABLE MAY BE UNUSABLE ON ANY SYSTEM WHERE THIS
 CERTIFICATE IS NOT INSTALLED.`n
`n
Continue at your own risk.
)
ifMsgBox Cancel
    ExitApp

if !A_IsAdmin
{
    if (in_file = "")
        in_file := "SkipWarning"
    cmd = "%A_ScriptFullPath%"
    if !A_IsCompiled
    {   ; Use A_AhkPath in case the "runas" verb isn't registered for ahk files.
        cmd = "%A_AhkPath%" %cmd%
    }
    Run *RunAs %cmd% "%in_file%" "%out_file%",, UseErrorLevel
    ExitApp
}

if (in_file = "" || in_file = "SkipWarning")
{
    ; Find AutoHotkey installation.
    RegRead InstallDir, HKEY_LOCAL_MACHINE, SOFTWARE\AutoHotkey, InstallDir
    if ErrorLevel && A_PtrSize=8
        RegRead InstallDir, HKLM, SOFTWARE\Wow6432Node\AutoHotkey, InstallDir

    ; Let user confirm or select file(s).
    FileSelectFile in_file, 1, %InstallDir%\AutoHotkey.exe
        , Select Source File, Executable Files (*.exe)
    if ErrorLevel
        ExitApp
    FileSelectFile out_file, S16, %in_file%
        , Select Destination File, Executable Files (*.exe)
    if ErrorLevel
        ExitApp
    user_specified_files := true
}

; Convert short paths to long paths.
Loop %in_file%, 0
    in_file := A_LoopFileLongPath
if (out_file = "")  ; i.e. only one file was given via command line.
    out_file := in_file
else
    Loop %out_file%, 0
        out_file := A_LoopFileLongPath

if Crypt.IsSigned(in_file)
{
    MsgBox 48,, Input file is already signed.  The script will now exit.
    ExitApp
}

if user_specified_files && !IsTrustedLocation(out_file)
{
    MsgBox 49,,
    (LTrim Join`s
    The path you have selected is not a trusted location. If you choose
    to continue, the uiAccess attribute will be set but will not have
    any effect until the file is moved to a trusted location. Trusted
    locations include \Program Files\ and \Windows\System32\.
    )
    ifMsgBox Cancel
        ExitApp
}

if (in_file = out_file)
{
    ; The following should typically work even if the file is in use:
    bak_file := in_file "~" A_Now ".bak"
    FileMove %in_file%, %bak_file%, 1
    if ErrorLevel
        Fail("Failed to rename selected file.")
    in_file := bak_file
}
FileCopy %in_file%, %out_file%, 1
if ErrorLevel
    Fail("Failed to copy file to destination.")


; Set the uiAccess attribute in the file's manifest to "true".
if !EXE_uiAccess_set(out_file, true)
    Fail("Failed to set uiAccess attribute in manifest.")


; Open the current user's "Personal" certificate store.
my := Cert.OpenStore(Cert.STORE_PROV_SYSTEM, 0, Cert.SYSTEM_STORE_CURRENT_USER, "wstr", "My")
if !my
    Warn("Failed to open 'Personal' certificate store.")

; Locate "AutoHotkey" certificate created by a previous run of this script.
ahk_cert := my.FindCertificates(0, Cert.FIND_SUBJECT_STR, "wstr", "AutoHotkey")[1]

if !ahk_cert
{
    ; Create key container.
    cr := Crypt.AcquireContext("AutoHotkey", 0, Crypt.PROV_RSA_FULL, Crypt.NEWKEYSET)
    if !cr
        Fail("Failed to create 'AutoHotkey' key container.")

    ; Generate key for certificate.
    key := cr.GenerateKey(Crypt.AT_SIGNATURE, 1024, Crypt.EXPORTABLE)

    ; Create simple certificate name.
    cn := new Cert.Name({CommonName: "AutoHotkey"})

    ; Set end time to 10 years from now.
    end_time := SystemTime.Now()
    end_time.Year += 10

    ; Create certificate using the parameters created above.
    ahk_cert := cr.CreateSelfSignCertificate(cn, 0, end_time)
    if !ahk_cert
        Fail("Failed to create 'AutoHotkey' certificate.")

    ; Add certificate to current user's "Personal" store so they won't
    ; need to create it again if they need to update the executable.
    if !(my.AddCertificate(ahk_cert, Cert.STORE_ADD_NEW))
        Warn("Failed to add certificate to 'Personal' store.")
    ; Proceed even if above failed, since it probably doesn't matter.
    
    ; Attempt to install certificate in trusted store.
    root := Cert.OpenStore(Cert.STORE_PROV_SYSTEM, 0, Cert.SYSTEM_STORE_LOCAL_MACHINE, "wstr", "Root")
    if !(root && root.AddCertificate(ahk_cert, Cert.STORE_ADD_USE_EXISTING))
    {
        if (%True% != "Silent")
        MsgBox 49,,
        (LTrim Join`s
        Failed to install certificate.  If you continue, the executable
        may become unusable until the certificate is manually installed.
        This can typically be done via Digital Signatures tab on the
        file's Properties dialog.
        )
        ifMsgBox Cancel
            ExitApp
    }
    
    key.Dispose()
    cr.Dispose()
}

; Sign the file.
if !SignFile(out_file, ahk_cert, "AutoHotkey")
    Fail("Failed to sign file.")


; In interactive mode, if not overwriting the original file, offer
; to create an additional context menu item for AHK files.
if (user_specified_files && in_file != out_file)
{
    RegRead uiAccessVerb, HKCR, AutoHotkeyScript\Shell\uiAccess\Command
    if ErrorLevel
    {
        MsgBox 3,, Register "Run Script with UI Access" context menu item?
        ifMsgBox Yes
        {
            RegWrite REG_SZ, HKCR, AutoHotkeyScript\Shell\uiAccess
                ,, Run with UI Access
            RegWrite REG_SZ, HKCR, AutoHotkeyScript\Shell\uiAccess\Command
                ,, "%out_file%" "`%1" `%*
        }
        ifMsgBox Cancel
            ExitApp
    }
}


; IsTrustedLocation
;   Returns true if path is a valid location for uiAccess="true".
IsTrustedLocation(path)
{
    ; http://msdn.microsoft.com/en-us/library/bb756929
    ; MSDN: "\Program Files\ and \windows\system32\ are currently the
    ;        two allowable protected locations."
    ; However, \Program Files (x86)\ also appears to be allowed.
    if InStr(path, A_ProgramFiles "\") = 1
        return true
    if InStr(path, A_WinDir "\System32\") = 1
        return true
    
    ; On 64-bit systems, if this script is 32-bit, A_ProgramFiles is
    ; %ProgramFiles(x86)%, otherwise it is %ProgramW6432%.  So check
    ; the opposite "Program Files" folder:
    EnvGet other, % A_PtrSize=8 ? "ProgramFiles(x86)" : "ProgramW6432"
    if (other != "" && InStr(path, other "\") = 1)
        return true
    
    return false
}


; EXE_uiAccess_set
;   Sets the uiAccess attribute in an executable file's manifest.
;     file  - Path of file.
;     value - New value; must be boolean (0 or 1).
EXE_uiAccess_set(file, value)
{
    ; Load manifest from EXE file.
    xml := ComObjCreate("Msxml2.DOMDocument")
    xml.async := false
    xml.setProperty("SelectionLanguage", "XPath")
    xml.setProperty("SelectionNamespaces"
        , "xmlns:v1='urn:schemas-microsoft-com:asm.v1' "
        . "xmlns:v3='urn:schemas-microsoft-com:asm.v3'")
    if !xml.load("res://" file "/#24/#1")
    {
        ; This will happen if the file doesn't exist or can't be opened,
        ; or if it doesn't have an embedded manifest.
        ErrorLevel := "load"
        return false
    }
    
    ; Check if any change is necessary. If the uiAccess attribute is
    ; not present, it is effectively "false":
    node := xml.selectSingleNode("/v1:assembly/v3:trustInfo/security"
                    . "/requestedPrivileges/requestedExecutionLevel")
    if ((node && node.getAttribute("uiAccess") = "true") = value)
    {
        ErrorLevel := "already set"
        return true
    }
    
    ; The follow "IF" section should be unnecessary for AutoHotkey_L.
    if !node
    {
        ; Get assembly node, which should always exist.
        if !last := xml.selectSingleNode("/v1:assembly")
        {
            ErrorLevel := "invalid manifest"
            return 0
        }
        for _, name in ["trustInfo", "security", "requestedPrivileges"
                        , "requestedExecutionLevel"]
        {
            if !(node := last.selectSingleNode("*[local-name()='" name "']"))
            {
                static NODE_ELEMENT := 1
                node := xml.createNode(NODE_ELEMENT, name
                                  , "urn:schemas-microsoft-com:asm.v3")
                last.appendChild(node)
            }
            last := node
        }
        ; Since the requestedExecutionLevel node didn't exist before,
        ; we must have just created it. Although this attribute *might*
        ; not actually be required, it seems best to set it:
        node.setAttribute("level", "asInvoker")
    }
    
    ; Set the uiAccess attribute!
    node.setAttribute("uiAccess", value ? "true" : "false")
    
    ; Retrieve XML text.
    xml := RTrim(xml.xml, "`r`n")
    
    ; Convert to UTF-8.
    VarSetCapacity(data, data_size := StrPut(xml, "utf-8") - 1)
    StrPut(xml, &data, "utf-8")
    
    ;
    ; Replace manifest resource.
    ;
    
    hupd := DllCall("BeginUpdateResource", "str", file, "int", false)
    if !hupd
    {
        ErrorLevel := "BeginUpdateResource"
        return false
    }
    
    ; Res type RT_MANIFEST (24), resource ID 1, language English (US)
    r := DllCall("UpdateResource", "ptr", hupd, "ptr", 24, "ptr", 1
                    , "ushort", 1033, "ptr", &data, "uint", data_size)
    
    if !DllCall("EndUpdateResource", "ptr", hupd, "int", !r)
    {
        ErrorLevel := "EndUpdateResource"
        return false
    }
    if !r ; i.e. above succeeded only in discarding the failed changes.
    {
        ErrorLevel := "UpdateResource"
        return false
    }
    ; Success!
    ErrorLevel := 0
    return true
}


Fail(msg)
{
    if (%True% != "Silent")
    MsgBox 16,, %msg%`n`nErrorLevel: %ErrorLevel%`nA_LastError: %A_LastError%
    ExitApp
}

Warn(msg)
{
    msg .= " (Err " ErrorLevel "; " A_LastError ")`n"
    OutputDebug %msg%
    FileAppend %msg%, *
}