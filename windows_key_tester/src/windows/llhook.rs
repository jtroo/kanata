//! Safe abstraction over the low-level windows keyboard hook API.

// This file is taken from kbremap with modifications.
// https://github.com/timokroeger/kbremap

use std::ptr;

use anyhow::Result;
use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;

/// Wrapper for the low-level keyboard hook API.
/// Automatically unregisters the hook when dropped.
pub struct KeyboardHook {
    handle: HHOOK,
}

impl KeyboardHook {
    /// Sets the low-level keyboard hook for this thread.
    ///
    /// Panics when a hook is already registered from the same thread.
    #[must_use = "The hook will immediatelly be unregistered and not work."]
    pub fn attach_hook() -> KeyboardHook {
        KeyboardHook {
            handle: unsafe {
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), ptr::null_mut(), 0)
                    .as_mut()
                    .expect("install low-level keyboard hook successfully")
            },
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.handle) };
    }
}

/// Key event received by the low level keyboard hook.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub code: u32,
    /// Key was released
    pub up: bool,
}

impl InputEvent {
    #[cfg(not(feature = "winiov2"))]
    fn from_hook_lparam(lparam: &KBDLLHOOKSTRUCT) -> Self {
        Self {
            code: lparam.vkCode,
            up: lparam.flags & LLKHF_UP != 0,
        }
    }

    #[cfg(feature = "winiov2")]
    fn from_hook_lparam(lparam: &KBDLLHOOKSTRUCT) -> Self {
        let extended = if lparam.flags & 0x1 == 0x1 { 0xE000 } else { 0 };
        let code = kanata_state_machine::oskbd::u16_to_osc((lparam.scanCode as u16) | extended)
            .map(Into::into)
            .unwrap_or(lparam.vkCode);
        Self {
            code,
            up: lparam.flags & LLKHF_UP != 0,
        }
    }
}

/// The actual WinAPI compatible callback.
unsafe extern "system" fn hook_proc(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hook_lparam = &*(lparam as *const KBDLLHOOKSTRUCT);
    let is_injected = hook_lparam.flags & LLKHF_INJECTED != 0;
    let key_event = InputEvent::from_hook_lparam(hook_lparam);
    log::info!("{code}, {wparam:?}, {is_injected}, {key_event:?}");
    CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
}

pub fn start() -> Result<()> {
    // Display debug and panic output when launched from a terminal.
    unsafe {
        use winapi::um::wincon::*;
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            panic!("Could not attach to console");
        }
    };
    native_windows_gui::init()?;
    // This callback should return `false` if the input event is **not** handled by the
    // callback and `true` if the input event **is** handled by the callback. Returning false
    // informs the callback caller that the input event should be handed back to the OS for
    // normal processing.
    let _kbhook = KeyboardHook::attach_hook();
    log::info!("hook attached, you can type now");
    // The event loop is also required for the low-level keyboard hook to work.
    native_windows_gui::dispatch_thread_events();
    Ok(())
}
