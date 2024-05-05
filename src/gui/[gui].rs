use crate::*;
use anyhow::{bail, Context, Result};
use clap::Parser;
use clap::{error::ErrorKind, CommandFactory};
use kanata_parser::cfg;
use simplelog::{format_description, *};

pub mod win;
pub use win::*;
pub mod win_nwg_ext;
use lib_main::*;
pub use win_nwg_ext::*;

pub use win_dbg_logger as log_win;
pub use win_dbg_logger::WINDBG_LOGGER;

use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};
pub static CFG: OnceLock<Arc<Mutex<Kanata>>> = OnceLock::new();
pub static GUI_TX: OnceLock<native_windows_gui::NoticeSender> = OnceLock::new();

/// Parse CLI arguments and initialize logging.
fn cli_init() -> Result<ValidatedArgs> {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(e) => {
            if *IS_TERM {
                // init loggers without config so '-help' "error" or real ones can be printed
                let mut log_cfg = ConfigBuilder::new();
                CombinedLogger::init(vec![
                    TermLogger::new(
                        LevelFilter::Debug,
                        log_cfg.build(),
                        TerminalMode::Mixed,
                        ColorChoice::AlwaysAnsi,
                    ),
                    log_win::windbg_simple_combo(LevelFilter::Debug),
                ])
                .expect("logger can init");
            } else {
                log_win::init();
                log::set_max_level(LevelFilter::Debug);
            } // doesn't panic
            match e.kind() {
                ErrorKind::DisplayHelp => {
                    let mut cmd = lib_main::Args::command();
                    let help = cmd.render_help();
                    info!("{help}");
                    log::set_max_level(LevelFilter::Off);
                    return Err(anyhow!(""));
                }
                _ => return Err(e.into()),
            }
        }
    };

    #[cfg(target_os = "macos")]
    if args.list {
        karabiner_driverkit::list_keyboards();
        std::process::exit(0);
    }

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
    log_cfg.set_time_format_custom(format_description!(
        version = 2,
        "[hour]:[minute]:[second].[subsecond digits:4]"
    ));
    if *IS_TERM {
        CombinedLogger::init(vec![
            TermLogger::new(
                log_lvl,
                log_cfg.build(),
                TerminalMode::Mixed,
                ColorChoice::AlwaysAnsi,
            ),
            log_win::windbg_simple_combo(log_lvl),
        ])
        .expect("logger can init");
    } else {
        CombinedLogger::init(vec![log_win::windbg_simple_combo(log_lvl)]).expect("logger can init");
    }
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

    if args.check {
        log::info!("validating config only and exiting");
        let status = match cfg::new_from_file(&cfg_paths[0]) {
            Ok(_) => 0,
            Err(e) => {
                log::error!("{e:?}");
                1
            }
        };
        std::process::exit(status);
    }

    #[cfg(target_os = "linux")]
    if let Some(wait) = args.wait_device_ms {
        use std::sync::atomic::Ordering;
        log::info!("Setting device registration wait time to {wait} ms.");
        oskbd::WAIT_DEVICE_MS.store(wait, Ordering::SeqCst);
    }

    Ok(ValidatedArgs {
        paths: cfg_paths,
        #[cfg(feature = "tcp_server")]
        tcp_server_address: args.tcp_server_address,
        #[cfg(target_os = "linux")]
        symlink_path: args.symlink_path,
        nodelay: args.nodelay,
    })
}

fn main_impl() -> Result<()> {
    let args = cli_init()?;
    let kanata_arc = Kanata::new_arc(&args)?;

    if CFG.set(kanata_arc.clone()).is_err() {
        warn!("Someone else set our ‘CFG’");
    }; // store a clone of cfg so that we can ask it to reset itself

    if !args.nodelay {
        info!("Sleeping for 2s. Please release all keys and don't press additional ones. Run kanata with --help to see how understand more and how to disable this sleep.");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // while also maintaining `tick()` calls to keyberon.

    let (tx, rx) = std::sync::mpsc::sync_channel(100);

    let (server, ntx, nrx) = if let Some(address) = {
        #[cfg(feature = "tcp_server")]
        {
            args.tcp_server_address
        }
        #[cfg(not(feature = "tcp_server"))]
        {
            None::<SocketAddrWrapper>
        }
    } {
        let mut server = TcpServer::new(address.into_inner(), tx.clone());
        server.start(kanata_arc.clone());
        let (ntx, nrx) = std::sync::mpsc::sync_channel(100);
        (Some(server), Some(ntx), Some(nrx))
    } else {
        (None, None, None)
    };

    native_windows_gui::init().context("Failed to init Native Windows GUI")?;
    let ui = build_tray(&kanata_arc)?;
    let gui_tx = ui.layer_notice.sender();
    if GUI_TX.set(gui_tx).is_err() {
        warn!("Someone else set our ‘GUI_TX’");
    };
    Kanata::start_processing_loop(kanata_arc.clone(), rx, ntx, args.nodelay);

    if let (Some(server), Some(nrx)) = (server, nrx) {
        #[allow(clippy::unit_arg)]
        Kanata::start_notification_loop(nrx, server.connections);
    }

    Kanata::event_loop(kanata_arc, tx, ui)?;

    Ok(())
}

pub fn lib_main_gui() {
    let _attach_console = *IS_CONSOLE;
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("{e}\n");
    }

    unsafe {
        FreeConsole();
    }
}
