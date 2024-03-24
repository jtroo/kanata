#![allow(unused_imports,unused_labels,unused_variables,unreachable_code,dead_code,non_upper_case_globals)]
//! Redirects output to the function provided by the entity supplying simulated input (e.g., AHK)
// todo: allow sharing numpad status to differentiate between vk enter and vk numpad enter
// todo: only press/release_key is implemented
use log::*;
use super::*;
use anyhow::{Result,Error,bail};

use crate::kanata::CalculatedMouseMove;
use kanata_parser::custom_action::*;

use std::io;

#[cfg(not(any(target_os="windows",target_os="macos")))]
use std::fmt;

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;

use std::sync::OnceLock;
use std::sync::Arc;
#[cfg(    feature="passthru_ahk")]
type CbOutEvFn = dyn Fn(i64,i64,i64) -> i64 + Send + Sync + 'static; // Rust wrapper func around external callback (transmuted into this) Ahk accept only i64 arguments (vk,sc,up)
#[cfg(not(feature="passthru_ahk"))]
type CbOutEvFn = dyn Fn(i64,i64,i64) -> i64 + Send + Sync + 'static;
struct FnOutEvWrapper {pub cb:Arc<CbOutEvFn>} // wrapper struct to store our callback in a thread-shareable manner
static OUTEVWRAP: OnceLock<FnOutEvWrapper> = OnceLock::new(); // ensure that our wrapper struct is created once (thread-safe)

/// Exported function: receives the address of the callback AHK function that accepts simulated output events
#[cfg(    feature = "passthru_ahk")]
#[no_mangle] pub extern "win64" fn set_out_ev_listener(cb_addr:c_longlong) -> LRESULT { //c_int = i32 c_longlong=i64
  // cbKanataOut(vk,sc,up) {return 1}: // All args are i64 (ahk doesn't support u64)
  // address: pointer-sized integer, equivalent to Int64 on ahk64
  // AHK uses x64 calling convention: todo: is this the same as win64? extern "C" also seems to work
  log::trace!("@set_out_ev_listener: got func address {}",cb_addr);
  let ptr_fn = cb_addr as *const (); // `as`-cast to a raw pointer before `transmute`ing to a function pointer. This avoids an integer-to-pointer `transmute`, which can be problematic. Transmuting between raw pointers and function pointers (i.e., two pointer types) is fine.
  let cb_out_ev = unsafe {std::mem::transmute::<*const (), fn(vk:i64,sc:i64,up:i64) -> i64>(ptr_fn)};
  OUTEVWRAP.get_or_init(|| {FnOutEvWrapper {cb:Arc::new(cb_out_ev)}});
  0
}

#[cfg(not(feature = "passthru_ahk"))]
#[no_mangle] pub extern "C" fn set_out_ev_listener(cb_addr:c_longlong) -> LRESULT { //c_int = i32 c_longlong=i64
  debug!("✗✗✗✗ unimplemented!");
  unimplemented!();
  0
}

fn send_out_ev(in_ev:InputEvent) -> Result<()> { // ext callback accepts vk:i64,sc:i64,up:i64
  #[cfg(feature="perf_logging")] let start = std::time::Instant::now();
  let key_event	= KeyEvent::try_from(in_ev); //{code:KEY_0,value:Press} //todo remove
  let vk:i64 = in_ev.code.into();
  let sc:i64 = 0;
  let up:i64 = in_ev.up.into();
  if let Some(fn_out_ev_wrapper) = OUTEVWRAP.get() {
    let handled = (&fn_out_ev_wrapper.cb)(vk,sc,up);
    if handled != 0 {
      #[cfg(    feature="perf_logging") ]
      debug!("🕐{}μs   ←←←✓fnHook {:?} {vk} {sc} {up}",(start.elapsed()).as_micros(),key_event);
      #[cfg(not(feature="perf_logging"))]
      debug!("   ←←←✓fnHook {:?} {vk} {sc} {up}",key_event);
      Ok(())} else {bail!("✗fnHook vk{} sc{} up{}",vk,sc,up)}
  } else {error!("✗✗✗unavailable");bail!("fnHook isn't available yet!")}
}

/// Handle for writing keys to the simulated input provider.
pub struct KbdOut {}

impl KbdOut {
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<Self, io::Error> {Ok(Self {})}
    #[cfg(target_os = "linux")]
    pub fn new(_: &Option<String>) -> Result<Self, io::Error> {Ok(Self {})}
    #[cfg(target_os = "linux")]
    pub fn write_raw(&mut self, event: InputEvent) -> Result<(),io::Error> {self.log.write_raw(event);debug!("out-raw:{event:?}");Ok(())}
    pub fn write    (&mut self, event: InputEvent) -> Result<(),io::Error> {
      let _ = send_out_ev(event);
      debug!("out:{event}");Ok(())}
    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let key_ev = KeyEvent::new(key, value);
        let event = {#[cfg(    target_os = "macos" )]{key_ev.try_into().unwrap()}
                     #[cfg(not(target_os = "macos"))]{key_ev.into()             }  };
        self.write(event)
      }
    pub fn write_code  (&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {debug!("out-code:{code};{value:?}");Ok(())}
    pub fn press_key   (&mut self, key: OsCode) -> Result<(),io::Error> {self.write_key(key,KeyValue::Press  )}
    pub fn release_key (&mut self, key: OsCode) -> Result<(),io::Error> {self.write_key(key,KeyValue::Release)}
    pub fn send_unicode(&mut self, c  : char  ) -> Result<(),io::Error> {debug!("outU:{c}");Ok(())}
    pub fn click_btn   (&mut self, btn: Btn   ) -> Result<(),io::Error> {debug!("out🖰:↓{btn:?}");Ok(())}
    pub fn release_btn (&mut self, btn: Btn   ) -> Result<(),io::Error> {debug!("out🖰:↑{btn:?}");Ok(())}
    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        debug!("scroll:{direction:?},{distance:?}");Ok(())}
    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
                         let (direction, distance) = ( mv.direction,  mv.distance);
            debug!("out🖰:move {direction:?},{distance:?}");Ok(())}
    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        for mv in moves {let (direction, distance) = (&mv.direction, &mv.distance);
            debug!("out🖰:move {direction:?},{distance:?}");}Ok(())}
    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {log::info!("out🖰:@{x},{y}");Ok(())}
}

#[cfg(not(any(target_os="windows",target_os="macos")))]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub code: u32,

    /// Key was released
    pub up: bool,
}
#[cfg(not(any(target_os="windows",target_os="macos")))]
impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let direction = if self.up { "↑" } else { "↓" };
        let key_name = KeyCode::from(OsCode::from(self.code));
        write!(f, "{}{:?}", direction, key_name)
    }
}

#[cfg(not(any(target_os="windows",target_os="macos")))]
impl InputEvent {
    pub fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        Self {
            code: code.into(),
            up: val.into(),
        }
    }
}

#[cfg(not(any(target_os="windows",target_os="macos")))]
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

#[cfg(not(any(target_os="windows",target_os="macos")))]
impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        Self {
            code: item.code.into(),
            up: item.value.into(),
        }
    }
}
#[cfg(target_os = "macos")]
impl KeyEvent {
    pub fn new(code: OsCode, value: KeyValue) -> Self {
        Self { code, value }
    }
}
