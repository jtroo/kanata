//! A function listener for keyboard input events replacing Windows keyboard hook API

use log::{info,debug,trace};
use core::fmt;
use std::cell::Cell;
use std::io;
use std::{mem, ptr};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;

use crate::kanata::CalculatedMouseMove;
use crate::oskbd::{KeyEvent, KeyValue};
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;
use kanata_keyberon::key_code::KeyCode;

pub const LLHOOK_IDLE_TIME_CLEAR_INPUTS: u64 = 60;

type HookFn = dyn FnMut(InputEvent) -> bool + Send + Sync + 'static;

static HOOK_CB:Lazy<Mutex<Option<Box<HookFn>>>> = Lazy::new(|| Mutex::new(None)); // store thread-safe hook callback with a mutex (can be called from an external process)

pub struct KeyboardHook {}  // reusing hook type for our listener
impl KeyboardHook { /// Sets input callback (panics if already registered)
  #[must_use = "The hook will immediatelly be unregistered and not work."]
  pub fn set_input_cb(callback: impl FnMut(InputEvent) -> bool + Send + Sync + 'static) {
    let mut cb_opt = HOOK_CB.lock(); assert!(cb_opt.take().is_none(),"Only 1 external listener is allowed!");
    *cb_opt = Some(Box::new(callback));
  }
}
impl Drop for KeyboardHook {fn drop(&mut self) {let mut cb_opt = HOOK_CB.lock(); cb_opt.take();}}

#[derive(Debug, Clone, Copy)] pub struct InputEvent { // Key event received by the low level keyboard hook.
  pub code: u32,
  pub up  : bool, /*Key was released*/  }
impl fmt::Display for InputEvent { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
  let direction = if self.up {"↑"} else {"↓"};
  let key_name = KeyCode::from(OsCode::from(self.code));
  write!(f, "{}{:?}", direction, key_name) }   }
impl InputEvent {
  pub fn from_vk_sc(vk:c_uint, sc:c_uint, up:c_int) -> Self {
    let code = if vk == (VK_RETURN as u32) { // todo: do a proper check for numpad enter, maybe 0x11c isn't universal
      match sc {0x11C => u32::from(vk_kpenter_fake)
        ,       _     => VK_RETURN as u32,}
    } else {vk};
    Self {code,up:(up!=0)}}
  pub fn from_oscode    (code:OsCode, val:KeyValue) -> Self { Self {
    code: code.into(),
    up  : val .into(),}}   }
impl TryFrom<InputEvent> for KeyEvent {
  type Error = ();
  fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
    Ok(Self {
      code : OsCode::from_u16(item.code as u16).ok_or(())?,
      value: match item.up {
        true  => KeyValue::Release,
        false => KeyValue::Press  ,},  })
  }   }
impl From<KeyEvent> for InputEvent {
  fn from(item: KeyEvent) -> Self { Self {
    code: item.code .into(),
    up  : item.value.into(),} }   }

/// Exported function: receives key input and uses event_loop's input event handler callback (which will in turn communicate via the internal kanata's channels to keyberon state machine etc.)
#[no_mangle] pub extern "C" fn input_ev_listener(vk:c_uint, sc:c_uint, up:c_int) -> LRESULT {
  #[cfg(feature="perf_logging")] let start = std::time::Instant::now();
  let key_event	= InputEvent::from_vk_sc(vk,sc,up); //{code:KEY_0,value:Press}

  let mut h_cbl	= HOOK_CB.lock(); // to access the closure we move its box out of the mutex and put it back after it returned
  if let Some(mut fnhook) = h_cbl.take() { // move our opt+boxed closure, replacing it with None, can't just .unwrap since Copy trait not implemented for dyn fnMut
    let handled = fnhook(key_event); // box(closure)() = closure()
    *h_cbl = Some(fnhook); // put our closure back
    if handled {
      #[cfg(    feature="perf_logging") ]
      debug!(" 🕐{}μs   →→→✓ {key_event} from {vk} sc={sc} up={up}",(start.elapsed()).as_micros());
      #[cfg(not(feature="perf_logging"))]
      debug!("   →→→✓ {key_event} from {vk} sc={sc} up={up}");
      1} else {0}
  } else {log::error!("fnHook processing key events isn't available yet"); 0}
}
