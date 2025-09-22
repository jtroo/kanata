#Requires AutoHotKey 2.1-alpha.4
/*
A short example of using Kanata as a library with AutoHotkey, first F8 press will load the library, second F8 press will activate Kanata for a few (ihDuration) seconds with 'f'/'j' turned into home row Shift mods.
The more useful script would not use F8, but f/j directly as hotkeys, but this owl hasn't been drawn yet...
Both Kanata and this script mainly output to Windows debug log, use github.com/smourier/TraceSpy to view it
Dependencies and config:
*/
  libPath   	:= "./"              	; kanata_passthru.dll @ this folder
  kanata_cfg	:= "./kanata_dll.kbd"	; kanata config @ this file location
  ihDuration	:= 10                	; seconds of activity after pressing F8
  dbg       	:= 1                 	; script's debug level (0 to silence some of its output)
  dbg_dll   	:= 1                  ; kanata's debug level (Err=1 Warn=2 Inf=3 Dbg=4 Trace=5)
/*
Brief overview of the architecture:
Setup:
  - AHK: configures cbKanataOut callback to Send keys out and shares its address with Kanata
  - Kanata: exports 4 functions
    - fnKanata_main: set up paths to config and initialize
    - fnKanata_in_ev: get input key events
    - K_output_ev_check: check if output key events exist
    - fnKanata_reset: reset Kanata's state without exiting
! AHK enables inputhook, so intercepts all keyboard input
← Redirects all intercepted input to Kanata, where we hit 2 limitations
  ✗ Kanata can't send keys out itself as it'll be intercepted by AHK's inputhook
  ✗ Kanata's thread that processes input can't call AHK function in the main thread since AHK is single-threaded
✓ Kanata opens an async channel from the input processing thread (with Keyberon state machine) to its main
  ← sends key out data back to the main thread
  → our script after sending input keys calls Kanata to read this channel until it's empty, and then Sends these keys out
*/

get_thread_id() {
  return DllCall("GetCurrentThreadId", "UInt")
}

F8::kanata_dll('vk77')
kanata_dll(vkC) {
  ; static K	:= keyConstant , vk := K._map, sc := K._mapsc  ; various key name constants, gets vk code to avoid issues with another layout
   ; , s    	:= helperString ; K.▼ = vk['▼']
  static is_init := false
   ,lErr:=1, lWarn:=2, lInf:=3, lDbg:=4, lTrace:=5, log_lvl := dbg_dll ; Kanata's
   ,last↓ := [0,0]
   ,id_thread := get_thread_id()
   ,Cvk_d := GetKeyVK(vkC), Csc_d := GetKeySC(vkC), token1 := 1, ih0 := 0 ; decimal value
   ,C↑ := false, cleanup := false ; track whether the trigger key has been released to not release it twice on kanata cleanup
  ; set up machinery for AHK and Kanata to communicate
  ,libNm            	:= "kanata_passthru"
  ,lib𝑓             	:= libNm '\' 'lib_kanata_passthru' ; receives AHK's address of AHK's cb KanataOut that accepts simulated output events
  ,lib𝑓input_ev     	:= libNm '\' 'input_ev_listener' ; receives key input and uses event_loop's input event handler callback (which will in turn communicate via the internal kanata's channels to keyberon state machine etc.)
  ,lib𝑓output_ev    	:= libNm '\' 'output_ev_check' ; checks if output event is ready (it's sent to our callback if it is)
  ,lib𝑓reset        	:= libNm '\' 'reset_kanata_state' ; reset kanata's state
  ,hModule          	:= DllCall("LoadLibrary", "Str",libPath libNm '.dll', "Ptr")  ; Avoids the need for DllCall in the loop to load the library
  ,fnKanata_main    	:= DllCall.Bind(lib𝑓, 'Ptr',unset, 'Str',unset, 'Int',unset)
  ,fnKanata_in_ev   	:= DllCall.Bind(lib𝑓input_ev, 'Int',unset , 'Int',unset,  'Int',unset)
  ,K_output_ev_check	:= DllCall.Bind(lib𝑓output_ev)
  ,fnKanata_reset   	:= DllCall.Bind(lib𝑓reset, 'Int',unset)
  static ih := InputHook("T" ihDuration " I1")
  , 🕐k_pre := A_TickCount
  , 🕐k_now := A_TickCount
  hooks := "hooks#: " gethookcount()

  addr_cbKanataOut := CallbackCreate(cbKanataOut)
  if not is_init {
    is_init := true

    ; setup inputhook callback functions
    ih.KeyOpt(  	'{All}','NSI')  ; N: Notify. OnKeyDown/OnKeyUp callbacks to be called each time the key is pressed
    ih.OnKeyDown	:= cbK↓.Bind(1)	;
    ih.OnKeyUp  	:= cbK↑.Bind(0)	;

    OutputDebug('¦' id_thread "¦registered inputhook with VisibleText=" ih.VisibleText " VisibleNonText=" ih.VisibleNonText "`nIlevel=" ih.MinSendLevel ' hooks#: ' gethookcount() ' →kanata addr#' addr_cbKanataOut)
    fnKanata_main(addr_cbKanataOut,kanata_cfg,log_lvl) ; setup kanata, passign ahk callback to accept out key events
    return
  }

  cbK↓(token,  ih,vk,sc) {
    static _d := 1, isUp := false, dir := (isUp?'↑':'↓')
    🕐k_pre := 🕐k_now
    🕐k_now := A_TickCount
    if (dbg>=_d) {
      dbgtxt := ''
      vk_hex := Format("vk{:x}",vk)
      key_name := GetKeyName(Format("vk{:x}",vk)) ; bugs with layouts, not english even if english is active
      dbgtxt .= "ih" dir (isSet(key_name)?key_name:'') "      🢥🄺: vk=" vk "¦" vk_hex " sc=" sc ' l' A_SendLevel " ¦" id_thread "¦"
      OutputDebug(dbgtxt)
    }
    isH := fnKanata_in_ev(vk,sc,isUp)
    dbgOut := ''
    for i in [4,4,4,5,5,5] { ; poll a key out channel@kanata) a few times to see if there are key events
      sleep(i)
      isOut := K_output_ev_check(), dbgOut.=isOut
      if (isOut < 0) { ; get as many keys as are available untill reception errors out
        break
      }
    } ;🔚∎🏁
    (dbg<_d+1)?'':(dbgtxt:='🏁ih' dir ' pos isH=' isH ' isOut=' dbgOut ' ' format(" 🕐Δ{:.3f}",A_TickCount - 🕐k_now) ' ' A_ThisFunc ' ¦' id_thread '¦', OutputDebug(dbgtxt))
  }
  cbK↑(token,  ih,vk,sc) {
    static _d := 1, isUp := true, dir := (isUp?'↑':'↓')
    🕐k_pre := 🕐k_now
    🕐k_now := A_TickCount
    if (dbg>=_d) {
      dbgtxt := ''
      vk_hex := Format("vk{:x}",vk)
      key_name := GetKeyName(Format("vk{:x}",vk)) ; bugs with layouts, not english even if english is active
      dbgtxt .= "ih" dir (isSet(key_name)?key_name:'') "      🢥🄺: vk=" vk "¦" vk_hex " sc=" sc ' l' A_SendLevel " ¦" id_thread "¦"
      OutputDebug(dbgtxt)
    }
    isH := fnKanata_in_ev(vk,sc,isUp)
    dbgOut := ''
    for i in [4,4,4,5,5,5] { ; poll a key out channel@kanata) a few times to see if there are key events
      sleep(i)
      isOut := K_output_ev_check(), dbgOut.=isOut
      if (isOut < 0) { ; get as many keys as are available until reception errors out
        break
      }
    }
    (dbg<_d+1)?'':(dbgtxt:='🏁ih' dir ' pos isH=' isH ' isOut=' dbgOut ' ' format(" 🕐Δ{:.3f}",A_TickCount - 🕐k_now) ' ' A_ThisFunc ' ¦' id_thread '¦', OutputDebug(dbgtxt))
  }
  ; set up machinery for AHK to receive data from kanata
  cbKanataOut(kvk,ksc,up) {
    ; static K	:= keyConstant, vk:=K._map, vkr:=K._mapr, vkl:=K._maplng, vkrl:=K._maprlng, vk→en:=vkrl['en'], sc:=K._mapsc  ; various key name constants, gets vk code to avoid issues with another layout
    static _d := 1, lvl_to := 0
    🕐1 := preciseTΔ()
    vk_hex := Format("vk{:x}",kvk)
    if not C↑ && up && (kvk=Cvk_d) {
      C↑ := true , (dbg<_d)?'':(OutputDebug('trigger key released'))
    }
    if cleanup && C↑ && up && (kvk=Cvk_d) { ; todo: check for physical position before excluding?
      (dbg<_d)?'':(OutputDebug("dupe release of trigger key on kanata's cleanup, ignore"))
      C↑ := false
      return
    }
    ; Critical ; todo: needed??? avoid being interrupted by itself (or any other thread)
    if (dbg>=_d) {
      dbgtxt := ''
      dir := (up?'↑':'↓')
      key_name := GetKeyName(vk_hex) ; bugs with layouts, not english even if english is active
      ; key_name := vk→en.Get(vk_hex,key_name_cur)
      hooks := "hooks#: " gethookcount()
      dbgtxt .= dir
    }
    if isSet(vk_hex) {
      (dbg<_d)?'':(dbgtxt .= key_name "       🄷🢦 : vk=" kvk '¦' vk_hex ' @l' A_SendLevel ' → ' lvl_to ' ' hooks ' ¦' id_thread '¦ ' A_ThisFunc, OutputDebug(dbgtxt))
      if up {
        ; SendEvent('{' vk_hex ' up}')
        SendInput('{' vk_hex ' up}')
      } else {
        ; SendEvent('{' vk_hex ' down}')
        SendInput('{' vk_hex ' down}')
      }
    } else {
      (dbg<_d)?'':(dbgtxt .= '✗name' "       🄷🢦 : vk=" kvk '¦' vk_hex ' @l' A_SendLevel ' → ' lvl_to ' ' hooks ' ¦' id_thread '¦ ' A_ThisFunc, OutputDebug(dbgtxt))
    }
    🕐2 := preciseTΔ(), 🕐Δ := 🕐2-🕐1
    if 🕐Δ > 0.5 {
      (dbg<_d+1)?'':(OutputDebug('🐢🏁 ' format(" 🕐Δ{:.3f}",🕐Δ) ' ¦' id_thread '¦ ' A_ThisFunc))
    } else {
      (dbg<_d+1)?'':(OutputDebug('🐇🏁 ' format(" 🕐Δ{:.3f}",🕐Δ) ' ¦' id_thread '¦ ' A_ThisFunc))
    }
    return 1
  }
  ; CallbackFree(cbKanataOut)

  if (Cvk_d) { ; modtap; send the activating hotkey to Kanata so it can take it into acount
    cbK↓(token1,ih0,Cvk_d,Csc_d)
  }
  ih.Start()		;
  ih.Wait() 		; Waits until the Input is terminated (InProgress is false)
  if (ih.EndReason  = "Timeout") { ; cleanup kanata's state
    ; key_name := GetKeyName(Format("vk{:x}",last↓[1]))
    OutputDebug('—`n`n——————————————— Timeout')
    🕐k_now := A_TickCount, 🕐Δ := 🕐k_now - 🕐k_pre
    cleanup := true
    res := fnKanata_reset(🕐Δ) ; reset kanata's state, progressing time to catch up, release held keys (even those physically held since reset is reset, so from kanata's perspective they should be released)
    cleanup := false
    dbgtxt := ''
    dbgtxt .= 'ih¦' 🕐Δ '🕐Δ timeout A_TimeSinceThisHotkey ' A_TimeSinceThisHotkey
    dbgOut := ''
    loop 10 { ; get the remaining out keys from kanata
      isOut := K_output_ev_check(), dbgOut.=isOut
      if (isOut < 0) {
        break
      }
    }
    OutputDebug(dbgtxt '`n`n——————————————— isOut=' dbgOut ' ')
  }

  ; DllCall("FreeLibrary", "Ptr",hModule)  ; to conserve memory, the DLL may be unloaded after using it
  hModule:=0
}

gethookcount() {
  if        (A_KeybdHookInstalled = 0) {
    return "_¦_"
  } else if (A_KeybdHookInstalled = 3) {
    return "✓¦✓"
  } else if (A_KeybdHookInstalled = 2) {
    return "_¦✓"
  } else if (A_KeybdHookInstalled = 1) {
    return "✓¦_"
  } else {
    return "?"
  }
}

preciseTΔ(n:=3) {
  static start := nativeFunc.GetSystemTimePreciseAsFileTime()
  t := round(     nativeFunc.GetSystemTimePreciseAsFileTime() - start,n)
  return t
}
class nativeFunc {
  static GetSystemTimePreciseAsFileTime() {
    /* learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime
    retrieves the current system date and time with the highest possible level of precision (<1us)
    FILETIME structure contains a 64-bit value representing the number of 100-nanosecond intervals since January 1, 1601 (UTC)
    100 ns  ->  0.1 µs  ->  0.001 ms  ->  0.00001 s
    1     sec  ->  1000 ms  ->  1000000 µs
    0.1   sec  ->   100 ms  ->   100000 µs
    0.001 sec  ->    10 ms  ->    10000 µs
    */
    static interval2sec := (10 * 1000 * 1000) ; 100ns * 10 → µs * 1000 → ms * 1000 → sec
    DllCall("GetSystemTimePreciseAsFileTime", "int64*",&ft:=0)
    return ft / interval2sec
  }
}
