use anyhow::{bail, Result};
use clap::{App, Arg};
use log::info;
use simplelog::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

mod cfg;
mod default_layers;
mod kbd_in;
mod kbd_out;
mod keys;
mod ktrl;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use ktrl::Ktrl;

const DEFAULT_CFG_PATH: &str = "./ktrl.kbd";
type CfgPath = PathBuf;

fn cli_init() -> Result<CfgPath> {
    let matches = App::new("ktrl")
        .version("0.0.1")
        .about("Unleashes your keyboard's full potential")
        .arg(
            Arg::with_name("cfg")
                .long("cfg")
                .value_name("CONFIG")
                .help(&format!(
                    "Path to your ktrl config file. Default: {}",
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

fn main_impl(cfg: CfgPath) -> Result<()> {
    let ktrl_arc = Ktrl::new_arc(cfg)?;
    info!("ktrl: Setup Complete");

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event_loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // as well as time-based events such as a tap-hold hold duration expiring.
    let (tx, rx) = mpsc::channel();
    Ktrl::start_processing_loop(ktrl_arc.clone(), rx);
    Ktrl::event_loop(ktrl_arc, tx)?;

    Ok(())
}

fn main() -> Result<()> {
    let args = cli_init()?;
    main_impl(args)
}
