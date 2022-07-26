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
mod tcp_server;

use clap::Parser;
use kanata::Kanata;
use tcp_server::TcpServer;

type CfgPath = PathBuf;

pub struct ValidatedArgs {
    path: CfgPath,
    port: Option<i32>,
    #[cfg(target_os = "linux")]
    symlink_path: Option<String>,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Configuration file to use with kanata
    #[clap(short, long, default_value = "kanata.kbd")]
    cfg: String,

    /// Port to run the notification server on
    #[clap(short, long)]
    port: Option<i32>,

    /// Enable debug logging
    #[clap(short, long)]
    debug: bool,

    /// Path of the symlink pointing to the newly-created device
    #[cfg(target_os = "linux")]
    #[clap(short, long)]
    symlink_path: Option<String>,

    /// Enable trace logging
    #[clap(short, long)]
    trace: bool,
}

/// Parse CLI arguments and initialize logging.
fn cli_init() -> Result<ValidatedArgs> {
    let args = Args::parse();

    let cfg_path = Path::new(&args.cfg);

    let log_lvl = match (args.debug, args.trace) {
        (_, true) => LevelFilter::Trace,
        (true, false) => LevelFilter::Debug,
        (false, false) => LevelFilter::Info,
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

    Ok(ValidatedArgs {
        path: cfg_path.into(),
        port: args.port,
        #[cfg(target_os = "linux")]
        symlink_path: args.symlink_path,
    })
}

fn main_impl() -> Result<()> {
    let args = cli_init()?;
    let kanata_arc = Kanata::new_arc(&args)?;
    info!("Kanata: config parsed");

    info!("Sleeping for 2s. Please release all keys and don't press additional ones.");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // while also maintaining `tick()` calls to keyberon.

    let (server, ntx, nrx) = if let Some(port) = args.port {
        let mut server = TcpServer::new(port);
        server.start(kanata_arc.clone());
        let (ntx, nrx) = crossbeam_channel::bounded(10);
        (Some(server), Some(ntx), Some(nrx))
    } else {
        (None, None, None)
    };

    let (tx, rx) = crossbeam_channel::bounded(10);
    Kanata::start_processing_loop(kanata_arc.clone(), rx, ntx);

    if let (Some(server), Some(nrx)) = (server, nrx) {
        Kanata::start_notification_loop(nrx, server.connections);
    }

    Kanata::event_loop(kanata_arc, tx)?;

    Ok(())
}

fn main() -> Result<()> {
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("{}", e);
    }
    eprintln!("\nPress any key to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
    ret
}
