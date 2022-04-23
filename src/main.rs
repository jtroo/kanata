use anyhow::{bail, Result};
use clap::{App, Arg};
use log::info;
use simplelog::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

mod cfg;
mod keys;
mod kanata;
mod layers;
mod oskbd;

use kanata::Kanata;

const DEFAULT_CFG_PATH: &str = "./kanata.kbd";
type CfgPath = PathBuf;

/// Parse CLI arguments and initialize logging.
fn cli_init() -> Result<CfgPath> {
    let matches = App::new("kanata")
        .version("0.0.1")
        .about("Unleashes your keyboard's full potential")
        .arg(
            Arg::with_name("cfg")
                .long("cfg")
                .value_name("CONFIG")
                .help(&format!(
                    "Path to your kanata config file. Default: {}",
                    DEFAULT_CFG_PATH
                ))
                .takes_value(true),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Enables debug level logging"),
        )
        .get_matches();

    let config_path = Path::new(matches.value_of("cfg").unwrap_or(DEFAULT_CFG_PATH));

    let log_lvl = match matches.is_present("debug") {
        true => LevelFilter::Debug,
        _ => LevelFilter::Info,
    };

    CombinedLogger::init(vec![TermLogger::new(
        log_lvl,
        Config::default(),
        TerminalMode::Mixed,
    )])
    .expect("Couldn't initialize the logger");

    if !config_path.exists() {
        bail!(
            "Could not find your config file ({})",
            config_path.to_str().unwrap_or("?")
        )
    }

    Ok(config_path.into())
}

#[cfg(target_os = "linux")]
fn main_impl(cfg: CfgPath) -> Result<()> {
    let kanata_arc = Kanata::new_arc(cfg)?;
    info!("kanata: Setup Complete");

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // while also maintaining `tick()` calls to keyberon.
    let (tx, rx) = mpsc::channel();
    Kanata::start_processing_loop(kanata_arc.clone(), rx);
    Kanata::event_loop(kanata_arc, tx)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn main_impl(cfg: CfgPath) -> Result<()> {
    // Need to use a thread with a larger stack size because Windows appears to have a lower
    // default stack size than Linux, which causes a stack overflow from generating the keyberon
    // Layout struct.
    //
    // I haven't played around with what the actual minimum should be, but 32MB seems reasonably
    // small anyway.
    let builder = std::thread::Builder::new()
        .name("kanata".into())
        .stack_size(32 * 1024 * 1024); // 32MB of stack space
    let handler = builder
        .spawn(|| {
            let kanata_arc = Kanata::new_arc(cfg).expect("Could not parse cfg");
            info!("kanata: Setup Complete");

            let (tx, rx) = mpsc::channel();
            Kanata::start_processing_loop(kanata_arc.clone(), rx);
            Kanata::event_loop(kanata_arc, tx).expect("Could not parse cfg");
        })
        .unwrap();

    handler.join().unwrap();
    Ok(())
}

fn main() -> Result<()> {
    let args = cli_init()?;
    main_impl(args)
}
