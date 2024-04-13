use std::time;
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

use std::cell::Cell;
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

use std::sync::mpsc::{Receiver};
thread_local! {pub static RX_KEY_EV_OUT:Cell<Option<Receiver<InputEvent>>> = Cell::default();} // Stores receiver for key data to be sent out for the current thread

fn lib_impl() -> Result<()> {
  let args = cli_init()?;
  let (tx_kout,rx_kout) = std::sync::mpsc::sync_channel(100);
  let cfg_arc = Kanata::new_arc(&args,Some(tx_kout))?; // new configuration from a file
  debug!("loaded {:?}",args.paths[0]);
  if CFG.set(cfg_arc.clone()).is_err() {warn!("Someone else set our ‘CFG’");}; // store a clone of cfg so that we can ask it to reset itself

  RX_KEY_EV_OUT.with(|state| {assert!(state.take().is_none(),"Only one channel to send keys out can be registered per thread.");
    state.set(Some(rx_kout));
  });

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

mod key_in;
mod key_out;
use crate::key_in::*;
use crate::key_out::*;

use log::*;
mod log_win;
#[no_mangle] pub extern "win64"
fn lib_kanata_passthru(cb_addr:c_longlong) -> LRESULT {
  log_init();
  let ret = set_cb_out_ev(cb_addr);
  if let Err(ref e) = ret {error!("couldn't register external key out event callback"); return 1};
  let ret = lib_impl();
  if let Err(ref e) = ret {error!("{e}\n"); return 1}
  0
}
