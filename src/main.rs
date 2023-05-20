use anyhow::{bail, Result};
use log::info;
use simplelog::*;
use std::path::PathBuf;

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
    paths: Vec<CfgPath>,
    port: Option<i32>,
    #[cfg(target_os = "linux")]
    symlink_path: Option<String>,
    nodelay: bool,
}

fn default_cfg() -> Vec<PathBuf> {
    let mut cfgs = Vec::new();

    let default = PathBuf::from("kanata.kbd");
    if default.is_file() {
        cfgs.push(default);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let fallback = config_dir.join("kanata").join("kanata.kbd");
        if fallback.is_file() {
            cfgs.push(fallback);
        }
    }

    cfgs
}

#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
/// kanata: an advanced software key remapper
///
/// kanata remaps key presses to other keys or complex actions depending on the
/// configuration for that key. You can find the guide for creating a config
/// file here:
///
///     https://github.com/jtroo/kanata/blob/main/docs/config.adoc
///
/// If you need help, please feel welcome to create an issue or discussion in
/// the kanata repository:
///
///     https://github.com/jtroo/kanata
struct Args {
    // Display different platform specific paths based on the target OS
    #[cfg_attr(
        target_os = "windows",
        doc = r"Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'C:\Users\user\AppData\Roaming\kanata\kanata.kbd'"
    )]
    #[cfg_attr(
        target_os = "macos",
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$HOME/Library/Application Support/kanata/kanata.kbd.'"
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$XDG_CONFIG_HOME/kanata/kanata.kbd'"
    )]
    #[arg(short, long, verbatim_doc_comment)]
    cfg: Option<Vec<PathBuf>>,

    /// Port to run the optional TCP server on. If blank, no TCP port will be
    /// listened on.
    #[arg(short, long, verbatim_doc_comment)]
    port: Option<i32>,

    /// Path for the symlink pointing to the newly-created device. If blank, no
    /// symlink will be created.
    #[cfg(target_os = "linux")]
    #[arg(short, long, verbatim_doc_comment)]
    symlink_path: Option<String>,

    /// Enable debug logging.
    #[arg(short, long)]
    debug: bool,

    /// Enable trace logging; implies --debug as well.
    #[arg(short, long)]
    trace: bool,

    /// Remove the startup delay on kanata. In some cases, removing the delay may cause keyboard
    /// issues on startup.
    #[arg(short, long)]
    nodelay: bool,
}

/// Parse CLI arguments and initialize logging.
fn cli_init() -> Result<ValidatedArgs> {
    let args = Args::parse();

    let cfg_paths = args.cfg.unwrap_or_else(default_cfg);

    let log_lvl = match (args.debug, args.trace) {
        (_, true) => LevelFilter::Trace,
        (true, false) => LevelFilter::Debug,
        (false, false) => LevelFilter::Info,
    };

    let mut log_cfg = ConfigBuilder::new();
    if let Err(e) = log_cfg.set_time_offset_to_local() {
        eprintln!("WARNING: could not set log TZ to local: {e:?}");
    };
    CombinedLogger::init(vec![TermLogger::new(
        log_lvl,
        log_cfg.build(),
        TerminalMode::Mixed,
        ColorChoice::AlwaysAnsi,
    )])
    .expect("logger can init");
    log::info!("kanata v{} starting", env!("CARGO_PKG_VERSION"));
    #[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
    log::info!("using LLHOOK+SendInput for keyboard IO");
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    log::info!("using the Interception driver for keyboard IO");

    if let Some(config_file) = cfg_paths.first() {
        if !config_file.exists() {
            bail!(
                "Could not find the config file ({})\nFor more info, pass the `-h` or `--help` flags.",
                cfg_paths[0].to_str().unwrap_or("?")
            )
        }
    } else {
        bail!("No config files provided\nFor more info, pass the `-h` or `--help` flags.");
    }

    Ok(ValidatedArgs {
        paths: cfg_paths,
        port: args.port,
        #[cfg(target_os = "linux")]
        symlink_path: args.symlink_path,
        nodelay: args.nodelay,
    })
}

fn main_impl() -> Result<()> {
    let args = cli_init()?;
    let kanata_arc = Kanata::new_arc(&args)?;

    if !args.nodelay {
        info!("Sleeping for 2s. Please release all keys and don't press additional ones.");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // while also maintaining `tick()` calls to keyberon.

    let (server, ntx, nrx) = if let Some(port) = args.port {
        let mut server = TcpServer::new(port);
        server.start(kanata_arc.clone());
        let (ntx, nrx) = std::sync::mpsc::channel();
        (Some(server), Some(ntx), Some(nrx))
    } else {
        (None, None, None)
    };

    let (tx, rx) = std::sync::mpsc::channel();
    Kanata::start_processing_loop(kanata_arc.clone(), rx, ntx, args.nodelay);

    if let (Some(server), Some(nrx)) = (server, nrx) {
        Kanata::start_notification_loop(nrx, server.connections);
    }

    #[cfg(target_os = "linux")]
    sd_notify::notify(true, &[sd_notify::NotifyState::Ready])?;

    Kanata::event_loop(kanata_arc, tx)?;

    Ok(())
}

fn main() -> Result<()> {
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("{e}\n");
    }
    eprintln!("\nPress enter to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
    ret
}
