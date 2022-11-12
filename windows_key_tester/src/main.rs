//! This program is intended to be similar to `evtest` but for Windows. It will read keyboard
//! events, print out the event info, then forward it the keyboard event as-is to the rest of the
//! operating system handling.

use anyhow::Result;
use log::info;
use simplelog::*;

use clap::Parser;

#[cfg(not(feature = "interception_driver"))]
mod llhook;
#[cfg(not(feature = "interception_driver"))]
use llhook::*;

#[cfg(feature = "interception_driver")]
mod interception;
#[cfg(feature = "interception_driver")]
use crate::interception::*;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Enable debug logging
    #[clap(short, long)]
    debug: bool,

    /// Enable trace logging (implies --debug as well)
    #[clap(short, long)]
    trace: bool,
}

/// Parse CLI arguments and initialize logging.
fn cli_init() {
    let args = Args::parse();

    let log_lvl = match (args.debug, args.trace) {
        (_, true) => LevelFilter::Trace,
        (true, false) => LevelFilter::Debug,
        (false, false) => LevelFilter::Info,
    };

    let mut log_cfg = ConfigBuilder::new();
    if let Err(e) = log_cfg.set_time_offset_to_local() {
        eprintln!("WARNING: could not set log TZ to local: {:?}", e);
    };
    CombinedLogger::init(vec![TermLogger::new(
        log_lvl,
        log_cfg.build(),
        TerminalMode::Mixed,
        ColorChoice::AlwaysAnsi,
    )])
    .expect("logger can init");
    log::info!("windows_key_tester v{} starting", env!("CARGO_PKG_VERSION"));
}

fn main_impl() -> Result<()> {
    cli_init();
    info!("Sleeping for 2s. Please release all keys and don't press additional ones.");
    std::thread::sleep(std::time::Duration::from_secs(2));
    start()?;
    Ok(())
}

fn main() -> Result<()> {
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("main got error {}", e);
    }
    eprintln!("\nPress any key to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
    ret
}
