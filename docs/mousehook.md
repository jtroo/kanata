
```rs

use winapi::um::{
    processthreadsapi::GetCurrentThreadId,
    winuser::{
        HOOKPROC, LPMSG,
        SetWindowsHookExA, UnhookWindowsHookEx, GetMessageA, PostThreadMessageA,
        WM_QUIT,
        WH_KEYBOARD_LL, WH_MOUSE_LL,
    }
};

pub fn setup_mouse_hook(&mut self) {
    use crate::hook::inner::low_level::mouse_procedure;
    self.mouse = Some(InnerHook::new(WH_MOUSE_LL, Some(mouse_procedure)));
}

pub struct InnerHook {
    hook_handle: Arc<Mutex<RawHook>>,
    thread_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

pub struct RawHook {
    pub raw_handle: HHOOK,
    pub thread_id: DWORD,
}

pub unsafe extern "system" fn mouse_procedure(
    code: INT,
    wm_mouse_param: WPARAM,
    win_hook_struct: LPARAM,
) -> LRESULT {
    // If code is less than zero, then the hook procedure
    // must pass the message to the CallNextHookEx function
    // without further processing and should return the value returned by CallNextHookEx.
    if code < 0 {
        unsafe {
            return CallNextHookEx(null_mut() as HHOOK, code, wm_mouse_param, win_hook_struct);
        }
    }

    let mice_hook_struct: *const MSLLHOOKSTRUCT = win_hook_struct as *mut _;
    let mouse_event = MouseEvent::new(wm_mouse_param, mice_hook_struct);
    let _ignore_error = GLOBAL_CHANNEL.send_mouse_event(mouse_event).is_err();

    CallNextHookEx(null_mut() as HHOOK, code, wm_mouse_param, win_hook_struct)
}
```
