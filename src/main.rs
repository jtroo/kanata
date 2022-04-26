use anyhow::{bail, Result};
use log::info;
use simplelog::*;
use std::path::{Path, PathBuf};

mod cfg;
mod custom_action;
mod kanata;
mod keys;
mod layers;
mod oskbd;

use clap::Parser;
use kanata::Kanata;

type CfgPath = PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Configuration file to use with kanata
    #[clap(short, long, default_value = "kanata.kbd")]
    cfg: String,

    /// Enable debug logging
    #[clap(short, long)]
    debug: bool,
}

/// Parse CLI arguments and initialize logging.
fn cli_init() -> Result<CfgPath> {
    let args = Args::parse();

    let cfg_path = Path::new(&args.cfg);

    let log_lvl = match args.debug {
        true => LevelFilter::Debug,
        _ => LevelFilter::Info,
    };

    CombinedLogger::init(vec![TermLogger::new(
        log_lvl,
        Config::default(),
        TerminalMode::Mixed,
    )])
    .expect("Couldn't initialize the logger");

    if !cfg_path.exists() {
        bail!(
            "Could not find your config file ({})",
            cfg_path.to_str().unwrap_or("?")
        )
    }

    Ok(cfg_path.into())
}

#[cfg(target_os = "linux")]
fn main_impl(cfg: CfgPath) -> Result<()> {
    let kanata_arc = Kanata::new_arc(cfg)?;
    info!("Kanata: config parsed");

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // while also maintaining `tick()` calls to keyberon.
    let (tx, rx) = crossbeam_channel::bounded(10);
    Kanata::start_processing_loop(kanata_arc.clone(), rx);
    Kanata::event_loop(kanata_arc, tx)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn main_impl(cfg: CfgPath) -> Result<()> {
    // Need to use a thread with a larger stack size because Windows appears to have a lower
    // default stack size than Linux, which causes a stack overflow from generating the keyberon
    // Layout struct.
    let builder = std::thread::Builder::new()
        .name("kanata".into())
        .stack_size(8 * 1024 * 1024); // 8MB of stack space, same as Linux default max
    let handler = builder
        .spawn(|| {
            let kanata_arc = Kanata::new_arc(cfg).expect("Could not parse cfg");
            info!("Kanata: config parsed");

            let (tx, rx) = crossbeam_channel::bounded(10);
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
