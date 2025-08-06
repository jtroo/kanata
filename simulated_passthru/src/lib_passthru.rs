#![cfg(all(feature = "passthru_ahk", target_os = "windows"))]
use anyhow::Result;
use anyhow::bail;

use kanata_state_machine::{oskbd::*, *};
use log::*;

fn log_init(max_lvl: &c_longlong) {
    log_win::init();
    let _a = log_win::set_thread_state(true);
    let log_lvl = match max_lvl {
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    };
    log::set_max_level(log_lvl);
}

use parking_lot::Mutex;
use std::cell::Cell;
use std::sync::{Arc, OnceLock};
static CFG: OnceLock<Arc<Mutex<Kanata>>> = OnceLock::new();

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
#[no_mangle]
pub extern "win64" fn reset_kanata_state(tick: c_longlong) -> LRESULT {
    debug!("                               ext →→→ reset_kanata_state");
    if let Some(cfg) = CFG.get() {
        if kanata::clean_state(cfg, tick.try_into().unwrap()).is_err() {
            debug!("✗ @ reset_kanata_state");
            return 1;
        };
    } else {
        debug!("✗ @ reset_kanata_state, no CFG");
        return 2;
    };
    0
}

use std::path::PathBuf;
/// Parse CLI arguments
fn cli_init(cfg_path: &str) -> Result<ValidatedArgs> {
    let cfg_file = PathBuf::from(&cfg_path);
    if !cfg_file.exists() {
        bail!("Could not find the config file ({:?})", cfg_file)
    }
    Ok(ValidatedArgs {
        paths: vec![cfg_file],
        #[cfg(feature = "tcp_server")]
        tcp_server_address: None, //todo: any need in a dll?
        nodelay: true,
        #[cfg(feature = "watch")]
        watch: false,
    })
}

use std::sync::mpsc::Receiver;
thread_local! {pub static RX_KEY_EV_OUT:Cell<Option<Receiver<InputEvent>>> = Cell::default();}
// Stores receiver for key data to be sent out for the current thread

fn lib_impl(cfg_path: &str) -> Result<()> {
    let args = cli_init(cfg_path)?;
    let (tx_kout, rx_kout) = std::sync::mpsc::channel(); //async channel
    let cfg_arc = Kanata::new_with_output_channel(&args, Some(tx_kout))?;
    // new configuration from a file
    debug!("loaded {:?}", args.paths[0]);
    if CFG.set(cfg_arc.clone()).is_err() {
        warn!("Someone else set our ‘CFG’");
    }; // store a clone of cfg so that we can ask it to reset itself

    RX_KEY_EV_OUT.with(|state| {
        assert!(
            state.take().is_none(),
            "Only one channel to send keys out can be registered per thread."
        );
        state.set(Some(rx_kout));
    });

    // Start a processing loop in another thread and run the event loop in this thread
    // The reason for two different event loops is that the "event loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // while also maintaining `tick()` calls to keyberon.
    let (tx, rx) = std::sync::mpsc::sync_channel(100);
    let ntx = None;
    Kanata::start_processing_loop(cfg_arc.clone(), rx, ntx, args.nodelay); // 2 handles keyboard
    // events while also
    // maintaining `tick()`
    // calls to keyberon

    Kanata::event_loop(cfg_arc, tx)?; // 1 only listens for keyboard events
    // (not a real loop, just registers callback closures
    // for external function to call at will)

    Ok(())
}

mod key_in;
mod key_out;

use crate::key_out::*;
use widestring::{
    U16CStr, //   0 U16/U32-CString wide version of the standard CString type
    U16Str,
    Utf16Str, // no0 UTF-16 encoded, growable owned string
    WideChar,
};

pub mod log_win;
#[no_mangle]
pub extern "win64" fn lib_kanata_passthru(
    cb_addr: c_longlong,
    cfg_path: &WideChar,
    max_lvl: c_longlong,
) -> LRESULT {
    log_init(&max_lvl);
    let ret = set_cb_out_ev(cb_addr);
    if let Err(ref _e) = ret {
        error!("couldn't register external key out event callback");
        return 1;
    };
    let cfg_path_wc = unsafe { U16CStr::from_ptr_str(cfg_path) }; // Constructs a wide C string
    // slice from a nul-terminated
    // string pointer
    let cfg_path_wx: &U16Str = cfg_path_wc.as_ustr(); // 16b wide string slice with undefined
    // encoding reject invalid UTF16 (skip check
    // with from_ustr_unchecked if certain input
    // is valid UTF16)
    let cfg_path_w: &Utf16Str = match Utf16Str::from_ustr(cfg_path_wx) {
        Ok(s) => s,
        Err(_e) => {
            error!("{_e}\n");
            return -1;
        }
    };
    let cfg_path_s: String = cfg_path_w.to_string(); // valid UTF16→conv is lossless&non-fallible

    let ret = lib_impl(&cfg_path_s);
    if let Err(ref e) = ret {
        error!("{e}\n");
        return -2;
    }
    0
}
