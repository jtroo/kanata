use crate::*;
use anyhow::{Context, anyhow};
use clap::{CommandFactory, error::ErrorKind};
use kanata_state_machine::gui::*;
use kanata_state_machine::*;
use std::fs::File;

/// Parse CLI arguments and initialize logging.
fn cli_init() -> Result<ValidatedArgs> {
    let noti_lvl = LevelFilter::Error; // min lvl above which to use Win system notifications
    let log_file_p = "kanata_log.txt";
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
                    log_win::windbg_simple_combo(LevelFilter::Debug, noti_lvl),
                ])
                .expect("logger can init");
            } else {
                CombinedLogger::init(vec![
                    log_win::windbg_simple_combo(LevelFilter::Debug, noti_lvl),
                    WriteLogger::new(
                        LevelFilter::Debug,
                        Config::default(),
                        File::create(log_file_p).unwrap(),
                    ),
                ])
                .expect("logger can init");
            }
            match e.kind() {
                ErrorKind::DisplayHelp => {
                    let mut cmd = Args::command();
                    let help = cmd.render_help();
                    info!("{help}");
                    log::set_max_level(LevelFilter::Off);
                    if !*IS_TERM {
                        // detached to open log still opened for writing
                        match open::that_detached(log_file_p) {
                            Ok(()) => {} // on the off-chance the user looks at WinDbg logs
                            Err(ef) => error!("failed to open {log_file_p} due to {ef:?}"),
                        }
                    }
                    return Err(anyhow!(""));
                }
                _ => {
                    if !*IS_TERM {
                        match open::that_detached(log_file_p) {
                            Ok(()) => {}
                            Err(ef) => error!("failed to open {log_file_p} due to {ef:?}"),
                        }
                    }
                    return Err(e.into());
                }
            }
        }
    };

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
            log_win::windbg_simple_combo(log_lvl, noti_lvl),
        ])
        .expect("logger can init");
    } else {
        CombinedLogger::init(vec![log_win::windbg_simple_combo(log_lvl, noti_lvl)])
            .expect("logger can init");
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

    Ok(ValidatedArgs {
        paths: cfg_paths,
        #[cfg(feature = "tcp_server")]
        tcp_server_address: args.tcp_server_address,
        nodelay: args.nodelay,
        #[cfg(feature = "watch")]
        watch: false,
    })
}

fn main_impl() -> Result<()> {
    let args = cli_init()?;
    let kanata_arc = Kanata::new_arc(&args)?;

    if CFG.set(kanata_arc.clone()).is_err() {
        warn!("Someone else set our ‘CFG’");
    }; // store a clone of cfg so that we can ask it to reset itself

    if !args.nodelay {
        info!(
            "Sleeping for 2s. Please release all keys and don't press additional ones. Run kanata with --help to see how understand more and how to disable this sleep."
        );
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
    let gui_cfg_tx = ui.cfg_notice.sender(); // allows notifying GUI on config reloads
    let gui_err_tx = ui.err_notice.sender(); // allows notifying GUI on erorrs (from logger)
    let gui_exit_tx = ui.exit_notice.sender(); // allows notifying GUI on app quit
    if GUI_TX.set(gui_tx).is_err() {
        warn!("Someone else set our ‘GUI_TX’");
    };
    if GUI_CFG_TX.set(gui_cfg_tx).is_err() {
        warn!("Someone else set our ‘GUI_CFG_TX’");
    };
    if GUI_ERR_TX.set(gui_err_tx).is_err() {
        warn!("Someone else set our ‘GUI_ERR_TX’");
    };
    if GUI_EXIT_TX.set(gui_exit_tx).is_err() {
        warn!("Someone else set our ‘GUI_EXIT_TX’");
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
