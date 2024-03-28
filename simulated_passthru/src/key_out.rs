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

use crate::oskbd::OUTEVWRAP;

/// Receives the address of the external app's callback function that accepts simulated output events and stores it in a static thread-safe OnceLock variable that can be then used by KbdOut which is called by the processing loop thread
#[cfg(    feature = "passthru_ahk")]
pub fn set_out_ev_listener(cb_addr:c_longlong) -> LRESULT { //c_int = i32 c_longlong=i64
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
fn set_out_ev_listener(cb_addr:c_longlong) -> LRESULT { //c_int = i32 c_longlong=i64
  debug!("✗✗✗✗ unimplemented!");
  unimplemented!();
  0
}
