#Requires AutoHotkey v2.0
Persistent true
listen_to_Kanata1()
listen_to_Kanata1() {
  static msgIDtxt := "kanata_4117d2917ccb4678a7a8c71a5ff898ed" ; must be set to the same value in Kanata
  static msgID := DllCall("RegisterWindowMessage", "Str",msgIDtxt), MSGFLT_ALLOW := 1
  if winID_self:=WinExist(A_ScriptHwnd) { ; need to allow some messages through due to AHK running with UIA access https://stackoverflow.com/questions/40122964/cross-process-postmessage-uipi-restrictions-and-uiaccess-true
    isRes := DllCall("ChangeWindowMessageFilterEx" ; learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changewindowmessagefilterex?redirectedfrom=MSDN
      , "Ptr",winID_self  	;i	HWND 		hwnd   	handle to the window whose UIPI message filter is to be modified
      ,"UInt",msgID       	;i	UINT 		message	message that the message filter allows through or blocks
      ,"UInt",MSGFLT_ALLOW	;i	DWORD		action
      , "Ptr",0)          	;io opt PCHANGEFILTERSTRUCT pChangeFilterStruct
  }
  OnMessage(msgID, setnv_mode, MaxThreads:=1)
  setnv_mode(wParam, lParam, msgID, hwnd) {
    if        wParam == 1 {
      curtime := FormatTime(,"dddd MMMM d, yyyy H:mm:ss")
    } else if wParam == 2 {
      curtime := FormatTime(,"yy")
    } else {
      curtime := "✗ wParam=" wParam " lParam=" lParam
    }
    SetKeyDelay(-1, 0)
    SendEvent(curtime)
  }
}

listen_to_Kanata2()
listen_to_Kanata2() {
  static msgIDtxt := "kanata_your_custom_message_string_unique_id" ; must be set to the same value in Kanata
  static msgID := DllCall("RegisterWindowMessage", "Str",msgIDtxt), MSGFLT_ALLOW := 1
  if winID_self:=WinExist(A_ScriptHwnd) { ; need to allow some messages through due to AHK running with UIA access https://stackoverflow.com/questions/40122964/cross-process-postmessage-uipi-restrictions-and-uiaccess-true
    isRes := DllCall("ChangeWindowMessageFilterEx" ; learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-changewindowmessagefilterex?redirectedfrom=MSDN
      , "Ptr",winID_self  	;i	HWND 		hwnd   	handle to the window whose UIPI message filter is to be modified
      ,"UInt",msgID       	;i	UINT 		message	message that the message filter allows through or blocks
      ,"UInt",MSGFLT_ALLOW	;i	DWORD		action
      , "Ptr",0)          	;io opt PCHANGEFILTERSTRUCT pChangeFilterStruct
  }
  OnMessage(msgID, setnv_mode, MaxThreads:=1)
  setnv_mode(wParam, lParam, msgID, hwnd) {
    SendInput("@kanata_your_custom_message_string_unique_id Unknown wParam=" wParam "lParam=" lParam)
  }
}
