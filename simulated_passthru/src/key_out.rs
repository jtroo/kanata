#![allow(unused_imports,unused_labels,unused_variables,unreachable_code,dead_code,non_upper_case_globals)]

use anyhow::Result;
use anyhow::{anyhow, bail};
use clap::Parser;
use kanata_parser::keys::str_to_oscode;
use kanata_state_machine::{oskbd::*, *};
use log::*;

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use std::sync::{Arc,OnceLock};
use parking_lot::Mutex;
use std::cell::Cell;

// use crate::oskbd::OUTEVWRAP;
// type CbOutEvFn = dyn Fn(i64,i64,i64) -> i64 + Send + Sync + 'static;
type CbOutEvFn = dyn Fn(i64,i64,i64) -> i64 + 'static;
thread_local! {static CBOUTEV_WRAP:Cell<Option<Box<CbOutEvFn>>> = Cell::default();} // Stores the hook callback for the current thread

/// - Get the address of AutoHotkey's callback function that accepts simulated output events (and sends them to the OS)
///   - `cbKanataOut(vk,sc,up) {return 1}` All args are i64 (AHK doesn't support u64)
/// - Store it in a static thread-local Cell (AHK is single-threaded, so we can only use this callback from the main thread). KbdOut will use a channel to send a message key event that will use call the fn from this Cell
/// address: pointer-sized integer, equivalent to Int64 on ahk64 (c_longlong=i64). Will be `as`-cast to a raw pointer before `transmute`ing to a function pointer to avoid an integer-to-pointer `transmute`, which can be problematic. Transmuting between raw pointers and function pointers (i.e., two pointer types) is fine.
/// AHK uses x64 calling convention: TODO: is this the same as win64? extern "C" also seems to work?
#[cfg(    feature="passthru_ahk")]
pub fn set_cb_out_ev(cb_addr:c_longlong) -> Result<()> {trace!("got func address {}",cb_addr);
  let ptr_fn    = cb_addr as *const ();
  let cb_out_ev = unsafe {std::mem::transmute::<*const (), fn(vk:i64,sc:i64,up:i64) -> i64>(ptr_fn)};
  CBOUTEV_WRAP.with(|state| {assert!(state.take().is_none(),"Only 1 callback can be registered per thread");
    state.set(Some(Box::new(cb_out_ev)));});
  Ok(())
}
#[cfg(not(feature="passthru_ahk"))]
fn set_cb_out_ev(cb_addr:c_longlong) -> Result<()>  {debug!("✗✗✗✗ unimplemented!");
  unimplemented!();
  Ok(())
}

use kanata_parser::keys::OsCode;

pub fn send_out_ev(in_ev:InputEvent) -> Result<()> { // ext callback accepts vk:i64,sc:i64,up:i64
  #[cfg(feature="perf_logging")] let start = std::time::Instant::now();
  let key_event	= KeyEvent::try_from(in_ev); //{code:KEY_0,value:Press} //todo remove
  debug!("@send_out_ev key_event={key_event:?}");
  let vk:i64 = in_ev.code.into();
  let sc:i64 = 0;
  let up:i64 = in_ev.up.into();

  let mut handled = 0i64;
  CBOUTEV_WRAP.with(|state| {
    if let Some(hook) = state.take() {handled = hook(vk,sc,up); state.set(Some(hook));}
  });
  #[cfg(    feature="perf_logging" )]debug!("🕐{}μs ←←←{} fnHookCC {key_event:?} {vk} {sc} {up}"
    ,      (start.elapsed()).as_micros(),if handled==1{"✓"}else{"✗"});
  #[cfg(not(feature="perf_logging"))]debug!(       "←←←{} fnHookCC {key_event:?} {vk} {sc} {up}"
    ,                                    if handled==1{"✓"}else{"✗"});
  Ok(())
}


