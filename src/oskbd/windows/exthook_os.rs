//! A function listener for keyboard input events replacing Windows keyboard hook API

use core::fmt;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use winapi::ctypes::*;
use winapi::um::winuser::*;

use crate::oskbd::{KeyEvent, KeyValue};
use kanata_keyberon::key_code::KeyCode;

use kanata_parser::keys::*;

pub const LLHOOK_IDLE_TIME_SECS_CLEAR_INPUTS: u64 = 60;

type HookFn = dyn FnMut(InputEvent) -> bool + Send + Sync + 'static;

pub static HOOK_CB: Lazy<Mutex<Option<Box<HookFn>>>> = Lazy::new(|| Mutex::new(None)); // store thread-safe hook callback with a mutex (can be called from an external process)

pub struct KeyboardHook {} // reusing hook type for our listener
impl KeyboardHook {
    /// Sets input callback (panics if already registered)
    pub fn set_input_cb(
        callback: impl FnMut(InputEvent) -> bool + Send + Sync + 'static,
    ) -> KeyboardHook {
        let mut cb_opt = HOOK_CB.lock();
        assert!(
            cb_opt.take().is_none(),
            "Only 1 external listener is allowed!"
        );
        *cb_opt = Some(Box::new(callback));
        KeyboardHook {}
    }
}
#[cfg(not(feature = "passthru_ahk"))] // unused KeyboardHook will be dropped, breaking our hook, disable it
impl Drop for KeyboardHook {
    fn drop(&mut self) {
        let mut cb_opt = HOOK_CB.lock();
        cb_opt.take();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    // Key event received by the low level keyboard hook.
    pub code: u32,
    pub up: bool, /*Key was released*/
}
impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let direction = if self.up { "↑" } else { "↓" };
        let key_name = KeyCode::from(OsCode::from(self.code));
        write!(f, "{direction}{key_name:?}")
    }
}
impl InputEvent {
    pub fn from_vk_sc(vk: c_uint, sc: c_uint, up: c_int) -> Self {
        let code = if vk == (VK_RETURN as u32) {
            // todo: do a proper check for numpad enter, maybe 0x11c isn't universal
            match sc {
                0x11C => u32::from(VK_KPENTER_FAKE),
                _ => VK_RETURN as u32,
            }
        } else {
            vk
        };
        Self {
            code,
            up: (up != 0),
        }
    }
    pub fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        Self {
            code: code.into(),
            up: val.into(),
        }
    }
}
impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();
    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        Ok(Self {
            code: OsCode::from_u16(item.code as u16).ok_or(())?,
            value: match item.up {
                true => KeyValue::Release,
                false => KeyValue::Press,
            },
        })
    }
}
impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        Self {
            code: item.code.into(),
            up: item.value.into(),
        }
    }
}
