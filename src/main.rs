#![cfg_attr(feature = "gui", windows_subsystem = "windows")]
// disable default console for a Windows GUI app
mod main_lib;

use anyhow::{Result, bail};
use clap::Parser;
use kanata_parser::cfg;
use kanata_state_machine::*;
use simplelog::{format_description, *};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
/// kanata: an advanced software key remapper
///
/// kanata remaps key presses to other keys or complex actions depending on the
/// configuration for that key. You can find the guide for creating a config
/// file here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc
///
/// If you need help, please feel welcome to create an issue or discussion in
/// the kanata repository: https://github.com/jtroo/kanata
struct Args {
    // Display different platform specific paths based on the target OS
    #[cfg_attr(
        target_os = "windows",
        doc = r"Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'C:\Users\user\AppData\Roaming\kanata\kanata.kbd'."
    )]
    #[cfg_attr(
        target_os = "macos",
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$HOME/Library/Application Support/kanata/kanata.kbd'."
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$XDG_CONFIG_HOME/kanata/kanata.kbd'."
    )]
    #[arg(short, long, verbatim_doc_comment)]
    cfg: Option<Vec<PathBuf>>,

    /// Read configuration from stdin instead of a file.
    #[arg(long, verbatim_doc_comment)]
    cfg_stdin: bool,

    /// Port or full address (IP:PORT) to run the optional TCP server on. If blank,
    /// no TCP port will be listened on.
    #[cfg(feature = "tcp_server")]
    #[arg(
        short = 'p',
        long = "port",
        value_name = "PORT or IP:PORT",
        verbatim_doc_comment
    )]
    tcp_server_address: Option<SocketAddrWrapper>,

    /// Path for the symlink pointing to the newly-created device. If blank, no
    /// symlink will be created.
    #[cfg(target_os = "linux")]
    #[arg(short, long, verbatim_doc_comment)]
    symlink_path: Option<String>,

    /// List the keyboards available for grabbing and exit.
    #[cfg(any(
        target_os = "macos",
        target_os = "linux",
        all(target_os = "windows", feature = "interception_driver")
    ))]
    #[arg(short, long)]
    list: bool,

    /// Disable logging, except for errors. Takes precedent over debug and trace.
    #[arg(short, long)]
    quiet: bool,

    /// Enable debug logging.
    #[arg(short, long)]
    debug: bool,

    /// Enable trace logging; implies --debug as well.
    #[arg(short, long)]
    trace: bool,

    /// Remove the startup delay.
    /// In some cases, removing the delay may cause keyboard issues on startup.
    #[arg(short, long, verbatim_doc_comment)]
    nodelay: bool,

    /// Milliseconds to wait before attempting to register a newly connected
    /// device. The default is 200.
    ///
    /// You may wish to increase this if you have a device that is failing
    /// to register - the device may be taking too long to become ready.
    #[cfg(target_os = "linux")]
    #[arg(short, long, verbatim_doc_comment)]
    wait_device_ms: Option<u64>,

    /// Validate configuration file and exit
    #[arg(long, verbatim_doc_comment)]
    check: bool,

    /// Log layer changes even if the configuration file has set the defcfg
    /// option to false. Useful if you are experimenting with a new
    /// configuration but want to default to no logging.
    #[arg(long, verbatim_doc_comment)]
    log_layer_changes: bool,

    /// Watch configuration files for changes and reload automatically
    #[arg(long, verbatim_doc_comment)]
    watch: bool,
}

#[cfg(not(feature = "gui"))]
mod cli {
    use super::*;

    /// Parse CLI arguments and initialize logging.
    fn cli_init() -> Result<(ValidatedArgs, Option<String>)> {
        let args = Args::parse();

        #[cfg(target_os = "macos")]
        if args.list {
            main_lib::list_devices_macos();
            std::process::exit(0);
        }

        #[cfg(target_os = "linux")]
        if args.list {
            main_lib::list_devices_linux();
            std::process::exit(0);
        }

        #[cfg(all(target_os = "windows", feature = "interception_driver"))]
        if args.list {
            main_lib::list_devices_windows();
            std::process::exit(0);
        }

        let config_string = if args.cfg_stdin {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Some(buf)
        } else {
            None
        };

        let cfg_paths = if config_string.is_none() {
            args.cfg.unwrap_or_else(default_cfg)
        } else {
            vec![]
        };

        let log_lvl = match (args.debug, args.trace, args.quiet) {
            (_, true, false) => LevelFilter::Trace,
            (true, false, false) => LevelFilter::Debug,
            (false, false, false) => LevelFilter::Info,
            (_, _, true) => LevelFilter::Error,
        };

        let mut log_cfg = ConfigBuilder::new();
        if let Err(e) = log_cfg.set_time_offset_to_local() {
            eprintln!("WARNING: could not set log TZ to local: {e:?}");
        };
        log_cfg.set_time_format_custom(format_description!(
            version = 2,
            "[hour]:[minute]:[second].[subsecond digits:4]"
        ));
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

        if config_string.is_none() {
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
        }

        if args.check {
            log::info!("validating config only and exiting");
            let status = if let Some(ref cfg_str) = config_string {
                use rustc_hash::FxHashMap;
                match cfg::new_from_str(cfg_str, FxHashMap::default()) {
                    Ok(_) => 0,
                    Err(e) => {
                        log::error!("{e:?}");
                        1
                    }
                }
            } else {
                match cfg::new_from_file(&cfg_paths[0]) {
                    Ok(_) => 0,
                    Err(e) => {
                        log::error!("{e:?}");
                        1
                    }
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

        if args.log_layer_changes {
            cfg_forced::force_log_layer_changes(true);
        }

        Ok((
            ValidatedArgs {
                paths: cfg_paths,
                #[cfg(feature = "tcp_server")]
                tcp_server_address: args.tcp_server_address,
                #[cfg(target_os = "linux")]
                symlink_path: args.symlink_path,
                nodelay: args.nodelay,
                #[cfg(feature = "watch")]
                watch: args.watch,
            },
            config_string,
        ))
    }

    pub(crate) fn main_impl() -> Result<()> {
        let (args, config_string) = cli_init()?;

        let kanata_arc = if let Some(cfg_str) = config_string {
            use rustc_hash::FxHashMap;
            let kanata = Kanata::new_from_str(&cfg_str, FxHashMap::default())?;
            std::sync::Arc::new(parking_lot::Mutex::new(kanata))
        } else {
            Kanata::new_arc(&args)?
        };

        if !args.nodelay {
            log::info!(
                "Sleeping for 2s. Please release all keys and don't press additional ones. Run kanata with --help to see how understand more and how to disable this sleep."
            );
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // Start a processing loop in another thread and run the event loop in this thread.
        //
        // The reason for two different event loops is that the "event loop" only listens for
        // keyboard events, which it sends to the "processing loop". The processing loop handles
        // keyboard events while also maintaining `tick()` calls to keyberon.

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

        Kanata::start_processing_loop(kanata_arc.clone(), rx, ntx, args.nodelay);

        if let (Some(server), Some(nrx)) = (server, nrx) {
            #[allow(clippy::unit_arg)]
            Kanata::start_notification_loop(nrx, server.connections);
        }

        // Start comprehensive file watcher if enabled (supports include files)
        #[cfg(feature = "watch")]
        if args.watch {
            if let Err(e) = crate::file_watcher::start_file_watcher(kanata_arc.clone()) {
                log::error!("Failed to start file watcher: {}", e);
            }
        }

        #[cfg(target_os = "linux")]
        sd_notify::notify(true, &[sd_notify::NotifyState::Ready])?;

        Kanata::event_loop(kanata_arc, tx)
    }
}

#[cfg(not(feature = "gui"))]
pub fn main() -> Result<()> {
    let ret = cli::main_impl();
    if let Err(ref e) = ret {
        log::error!("{e}\n");
    }
    eprintln!("\nPress enter to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
    ret
}

#[cfg(all(feature = "gui", target_os = "windows"))]
fn main() {
    main_lib::win_gui::lib_main_gui();
}
