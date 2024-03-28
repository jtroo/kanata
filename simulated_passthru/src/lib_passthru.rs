use anyhow::Result;
use anyhow::{anyhow, bail};
use clap::Parser;
use kanata_parser::keys::str_to_oscode;
use kanata_state_machine::{oskbd::*, *};
use log::*;


fn log_init() {
  let _ = log_win::init();
  let a = log_win::set_thread_state(true);
  log::set_max_level(log::LevelFilter::Trace);
}

use std::sync::{Arc,OnceLock};
use parking_lot::Mutex;
static CFG:OnceLock<Arc<Mutex<Kanata>>> = OnceLock::new();

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
#[no_mangle] pub extern "win64" fn reset_kanata_state(tick:c_longlong) -> LRESULT {
  debug!("                               ext →→→ reset_kanata_state");
  if let Some(cfg) = CFG.get() {
    if kanata::clean_state(&cfg,tick.try_into().unwrap()).is_err()	{debug!("✗ @ reset_kanata_state"        );return 1};
  } else                                                          	{debug!("✗ @ reset_kanata_state, no CFG");return 2};
  0
}

use std::path::PathBuf;
/// Parse CLI arguments
fn cli_init() -> Result<ValidatedArgs> {
  let cfg_file = PathBuf::from(r#"./sim.kbd"#);
  if !cfg_file.exists() {bail!("Could not find the config file ({:?})"    ,cfg_file)}
  Ok(ValidatedArgs {paths:vec![cfg_file], nodelay:true},)
}

fn lib_impl() -> Result<()> {
  log_init();
  let args = cli_init()?;
  let cfg_arc = Kanata::new_arc(&args)?; // new configuration from a file
  if CFG.set(cfg_arc.clone()).is_err() {warn!("Someone else set our ‘CFG’");}; // store a clone of cfg so that we can ask it to reset itself

  // Start a processing loop in another thread and run the event loop in this thread
  // The reason for two different event loops is that the "event loop" only listens for keyboard events, which it sends to the "processing loop". The processing loop handles keyboard events while also maintaining `tick()` calls to keyberon.
  let (tx,rx) = std::sync::mpsc::sync_channel(100);
  let ntx = None;
  Kanata::start_processing_loop(cfg_arc.clone(), rx, ntx, args.nodelay); // 2 handles keyboard events while also maintaining `tick()` calls to keyberon

  Kanata::event_loop(cfg_arc, tx)?; // 1 only listens for keyboard events (not a real loop, just registers callback closures for external function to call at will)

  Ok(())
}

use kanata_parser::keys::OsCode;
use std::panic::catch_unwind;


use crate::oskbd::OUTEVWRAP;
/// Receives the address of the external app's callback function that accepts simulated output events
#[cfg(    feature = "passthru_ahk")]
fn set_out_ev_listener(cb_addr:c_longlong) -> LRESULT { //c_int = i32 c_longlong=i64
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

use log::{debug, info};
mod log_win;
#[no_mangle] pub extern "win64"
fn lib_kanata_passthru(cb_addr:c_longlong) -> LRESULT {
  let reg = set_out_ev_listener(cb_addr);
  if reg == 1 {error!("couldn't register external key out event callback"); return 1}
  let ret = lib_impl();
  if let Err(ref e) = ret {error!("{e}\n"); return 1}
  0
}
