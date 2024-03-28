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


use log::{debug, info};
mod log_win;
#[no_mangle] pub extern "C"
fn sim_evt() -> bool {
  let ret = lib_impl();
  if let Err(ref e) = ret {log::error!("{e}\n"); return false}
  // ret
  // let result = catch_unwind(|| {
  //   panic!("Oops!");
  // });
  // match result {
  //   Ok(_) => dbgwin(&format!("successfully caught unwind")),
  //   Err(cause) => dbgwin(&format!("panic! cause={:?}",cause)),
  // }

  // let key_code = str_to_oscode("val").unwrap_or(OsCode::KEY_RESERVED);
  // debug!("key_code={key_code:?}");
  debug!("✗✗✗ sim_evt finished");
  true
}
