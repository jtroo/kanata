//! Safe abstraction over the low-level windows keyboard hook API.

use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::{mem, ptr};

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;

use crate::keys::*;

type HookFn<'a> = dyn FnMut(InputEvent) -> bool + 'a;

thread_local! {
    /// Stores the hook callback for the current thread.
    static HOOK_STATE: RefCell<HookState> = RefCell::default();
}

#[derive(Default)]
struct HookState {
    hook: Option<Box<HookFn<'static>>>,
}

/// Wrapper for the low-level keyboard hook API.
/// Automatically unregisters the hook when dropped.
pub struct KeyboardHook<'a> {
    handle: HHOOK,
    lifetime: PhantomData<&'a ()>,
}

impl<'a> KeyboardHook<'a> {
    /// Sets the low-level keyboard hook for this thread.
    ///
    /// The closure receives key press and key release events. When the closure
    /// returns `None` they key event is not modified and forwarded to processes
    /// is if nothing happened. To ignore a key event or to remap it to another
    /// key return a [`KeyAction`].
    ///
    /// Character actions are sent with a single virtual key event if the character
    /// is available on the current system keyboard layout.
    /// Uses `VK_PACKET` to remap a key to Unicode codepoint if no dedicated key
    /// for that character exists.
    ///
    /// Panics when a hook is already registered from the same thread.
    #[must_use = "The hook will immediatelly be unregistered and not work."]
    pub fn set_input_cb(callback: impl FnMut(InputEvent) -> bool + 'a) -> KeyboardHook<'a> {
        HOOK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            assert!(
                state.hook.is_none(),
                "Only one keyboard hook can be registered per thread."
            );

            // The rust compiler needs type annotations to create a trait object rather than a
            // specialized boxed closure so that we can use transmute in the next step.
            let boxed_cb: Box<HookFn<'a>> = Box::new(callback);

            // Safety: Transmuting to 'static lifetime is required to put the closure in thread
            // local storage. It is safe to do so because we properly unregister the hook on drop
            // after which the global (thread local) variable `HOOK` will not be acccesed anymore.
            state.hook = Some(unsafe { mem::transmute(boxed_cb) });

            KeyboardHook {
                handle: unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), ptr::null_mut(), 0)
                        .as_mut()
                        .expect("Failed to install low-level keyboard hook.")
                },
                lifetime: PhantomData,
            }
        })
    }
}

impl<'a> Drop for KeyboardHook<'a> {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.handle) };
        HOOK_STATE.with(|state| state.take());
    }
}

/// Key event received by the low level keyboard hook.
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub code: u32,

    /// Key was released
    pub up: bool,

    /// Time in milliseconds since boot.
    pub time: u32,
}

impl InputEvent {
    fn from_hook_lparam(lparam: &KBDLLHOOKSTRUCT) -> Self {
        Self {
            code: lparam.vkCode,
            up: lparam.flags & LLKHF_UP != 0,
            time: lparam.time,
        }
    }

    fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        Self {
            code: code.into(),
            up: val.into(),
            time: 0,
        }
    }
}

/// The actual WinAPI compatible callback.
unsafe extern "system" fn hook_proc(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION {
        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
    }

    let hook_lparam = &*(lparam as *const KBDLLHOOKSTRUCT);
    let key_event = InputEvent::from_hook_lparam(hook_lparam);
    let injected = hook_lparam.flags & LLKHF_INJECTED != 0;

    // `SendInput()` internally calls the hook function. Filter out injected events
    // to prevent recursion and potential stack overflows if our remapping logic
    // sent the injected event.
    if injected {
        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
    }

    let handled = HOOK_STATE.with(|state| {
        // The mutable reference can be taken as long as we properly prevent recursion
        // by dropping injected events.
        let mut state = state.borrow_mut();

        // The unwrap cannot fail, because windows only calls this function after
        // registering the hook (before which we have set [`HOOK_STATE`]).
        state.hook.as_mut().unwrap()(key_event)
    });

    if handled {
        -1
    } else {
        CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
    }
}

pub fn send_key(key: InputEvent) {
    log::info!("sending: {:?}", key);
    unsafe {
        let mut inputs: [INPUT; 2] = mem::zeroed();

        let mut kb_input = key_input_from_event(key);
        kb_input.wVk = key.code as u16;

        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = kb_input;

        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}

/// Handle for writing keys to the OS.
pub struct KbdOut {}

impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {})
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        send_key(event);
        Ok(())
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let event = InputEvent::from_oscode(key, value);
        self.write(event)
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }
}

fn key_input_from_event(key: InputEvent) -> KEYBDINPUT {
    let mut kb_input: KEYBDINPUT = unsafe { mem::zeroed() };
    if key.up {
        kb_input.dwFlags |= KEYEVENTF_KEYUP;
    }
    kb_input.time = key.time;
    kb_input
}
