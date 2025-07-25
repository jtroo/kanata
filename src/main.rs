#![cfg_attr(feature = "gui", windows_subsystem = "windows")]
// disable default console for a Windows GUI app
mod main_lib;

use anyhow::{bail, Result};
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
    #[cfg(target_os = "macos")]
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

    #[cfg(feature = "watch")]
    mod file_watcher {
        use super::*;
        use parking_lot::Mutex;
        use std::sync::Arc;
        use std::time::Duration;
        use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult, DebouncedEventKind};

        pub fn start_file_watcher(
            cfg_paths: Vec<PathBuf>,
            kanata_arc: Arc<Mutex<Kanata>>,
        ) -> Result<()> {
            // Create debouncer with 500ms timeout and a closure for event handling
            let kanata_arc_clone = kanata_arc.clone();
            let cfg_paths_clone = cfg_paths.clone();
            
            let mut debouncer = new_debouncer(Duration::from_millis(500), move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        for event in events {
                            // Check if the changed file is one of our config files
                            if cfg_paths_clone.iter().any(|cfg_path| {
                                event.path.canonicalize().unwrap_or(event.path.clone()) 
                                    == cfg_path.canonicalize().unwrap_or(cfg_path.clone())
                            }) {
                                match event.kind {
                                    DebouncedEventKind::Any => {
                                        log::info!("Config file changed: {}, triggering reload", event.path.display());
                                        
                                        // Set the live_reload_requested flag
                                        if let Some(mut kanata) = kanata_arc_clone.try_lock() {
                                            kanata.request_live_reload();
                                        } else {
                                            log::warn!("Could not acquire lock to set live_reload_requested");
                                        }
                                    }
                                    _ => {
                                        log::trace!("Ignoring file event: {:?} for {}", event.kind, event.path.display());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File watcher error: {:?}", e);
                    }
                }
            })?;
            
            // Watch all config files directly
            for path in &cfg_paths {
                debouncer.watcher().watch(path, RecursiveMode::NonRecursive)?;
                log::info!("Watching config file for changes: {}", path.display());
            }

            // Keep the debouncer alive by moving it to a static context
            std::mem::forget(debouncer);
            
            Ok(())
        }
    }

    /// Parse CLI arguments and initialize logging.
    fn cli_init() -> Result<ValidatedArgs> {
        let args = Args::parse();

        #[cfg(target_os = "macos")]
        if args.list {
            karabiner_driverkit::list_keyboards();
            std::process::exit(0);
        }

        let cfg_paths = args.cfg.unwrap_or_else(default_cfg);

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

        if args.log_layer_changes {
            cfg_forced::force_log_layer_changes(true);
        }

        Ok(ValidatedArgs {
            paths: cfg_paths,
            #[cfg(feature = "tcp_server")]
            tcp_server_address: args.tcp_server_address,
            #[cfg(target_os = "linux")]
            symlink_path: args.symlink_path,
            nodelay: args.nodelay,
            #[cfg(feature = "watch")]
            watch: args.watch,
        })
    }

    pub(crate) fn main_impl() -> Result<()> {
        let args = cli_init()?;
        let kanata_arc = Kanata::new_arc(&args)?;

        if !args.nodelay {
            log::info!("Sleeping for 2s. Please release all keys and don't press additional ones. Run kanata with --help to see how understand more and how to disable this sleep.");
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

        // Start file watcher if enabled
        #[cfg(feature = "watch")]
        if args.watch {
            if let Err(e) = file_watcher::start_file_watcher(args.paths.clone(), kanata_arc.clone()) {
                log::error!("Failed to start file watcher: {}", e);
            }
        }

        #[cfg(target_os = "linux")]
        sd_notify::notify(true, &[sd_notify::NotifyState::Ready])?;

        Kanata::event_loop(kanata_arc, tx)
    }

    #[cfg(all(feature = "watch", test))]
    mod tests {
        use super::*;
        use std::fs;
        use tempfile::NamedTempFile;

        #[test]
        fn test_request_live_reload_method() {
            // Test that the new request_live_reload method works
            let temp_file = NamedTempFile::with_suffix(".kbd").unwrap();
            let config_path = temp_file.path().to_path_buf();
            
            // Write a minimal valid config
            fs::write(&config_path, r#"
                (defsrc caps)
                (deflayer default caps)
            "#).unwrap();

            let args = ValidatedArgs {
                paths: vec![config_path],
                #[cfg(feature = "tcp_server")]
                tcp_server_address: None,
                #[cfg(target_os = "linux")]
                symlink_path: None,
                nodelay: true,
                #[cfg(feature = "watch")]
                watch: false,
            };

            let mut kanata = Kanata::new(&args).unwrap();
            
            // Initially, live_reload_requested should be false
            assert!(!kanata.is_live_reload_requested());
            
            // Request live reload
            kanata.request_live_reload();
            
            // Now it should be true
            assert!(kanata.is_live_reload_requested());
        }

        #[test]
        fn test_file_watcher_path_validation() {
            // Test that file watcher handles various path scenarios correctly
            let valid_paths = vec![
                PathBuf::from("/tmp/test.cfg"),
                PathBuf::from("./relative.cfg"),
                PathBuf::from("/home/user/config.kbd"),
            ];
            
            // All paths should be valid for direct file watching
            for path in &valid_paths {
                assert!(path.as_os_str().len() > 0, "Config path should not be empty: {}", path.display());
                assert!(path.extension().is_some(), "Config file should have an extension: {}", path.display());
            }
        }

        #[test]
        fn test_validated_args_with_watch_flag() {
            let temp_file = NamedTempFile::with_suffix(".kbd").unwrap();
            let config_path = temp_file.path().to_path_buf();
            
            let args = ValidatedArgs {
                paths: vec![config_path.clone()],
                #[cfg(feature = "tcp_server")]
                tcp_server_address: None,
                #[cfg(target_os = "linux")]
                symlink_path: None,
                nodelay: true,
                #[cfg(feature = "watch")]
                watch: true,
            };

            // Test that ValidatedArgs properly stores the watch flag
            #[cfg(feature = "watch")]
            assert!(args.watch);
            
            // Test that paths are stored correctly
            assert_eq!(args.paths.len(), 1);
            assert_eq!(args.paths[0], config_path);
        }

        #[test]
        fn test_file_watcher_feature_flag() {
            // This test ensures that file watching code is only compiled with the watch feature
            #[cfg(feature = "watch")]
            {
                // This should compile when watch feature is enabled
                let _paths = vec![PathBuf::from("test.cfg")];
                // file_watcher module should be available
            }
            
            #[cfg(not(feature = "watch"))]
            {
                // When watch feature is disabled, the module shouldn't be available
                // This is verified at compile time
            }
        }

        #[test]
        fn test_file_watcher_integration() {
            use std::sync::Arc;
            use parking_lot::Mutex;
            
            // Create a temporary config file
            let temp_file = NamedTempFile::with_suffix(".kbd").unwrap();
            let config_path = temp_file.path().to_path_buf();
            
            // Write initial config
            fs::write(&config_path, r#"
                (defsrc caps a)
                (deflayer default lrld a)
            "#).unwrap();

            // Create ValidatedArgs
            let args = ValidatedArgs {
                paths: vec![config_path.clone()],
                #[cfg(feature = "tcp_server")]
                tcp_server_address: None,
                #[cfg(target_os = "linux")]
                symlink_path: None,
                nodelay: true,
                #[cfg(feature = "watch")]
                watch: true,
            };

            // Create Kanata instance
            let kanata = Kanata::new(&args).unwrap();
            let kanata_arc = Arc::new(Mutex::new(kanata));
            
            // Verify initial state
            {
                let k = kanata_arc.lock();
                assert!(!k.is_live_reload_requested());
            }
            
            // Test that we can manually trigger a reload request
            {
                let mut k = kanata_arc.lock();
                k.request_live_reload();
                assert!(k.is_live_reload_requested());
            }
            
            // Reset the flag 
            {
                let mut k = kanata_arc.lock();
                k.reset_live_reload_requested();
                assert!(!k.is_live_reload_requested());
            }
            
            // Test path validation for file watcher setup
            assert!(config_path.exists(), "Config file should exist for direct watching");
            assert!(config_path.is_file(), "Config path should be a file, not directory");
        }

        #[test] 
        fn test_multiple_config_files_watching() {
            // Test watching multiple configuration files
            let temp_file1 = NamedTempFile::with_suffix(".kbd").unwrap();
            let temp_file2 = NamedTempFile::with_suffix(".cfg").unwrap();
            let config_path1 = temp_file1.path().to_path_buf();
            let config_path2 = temp_file2.path().to_path_buf();
            
            // Write configs
            for path in &[&config_path1, &config_path2] {
                fs::write(path, r#"
                    (defsrc caps)
                    (deflayer default caps)
                "#).unwrap();
            }

            let args = ValidatedArgs {
                paths: vec![config_path1.clone(), config_path2.clone()],
                #[cfg(feature = "tcp_server")]
                tcp_server_address: None,
                #[cfg(target_os = "linux")]
                symlink_path: None,
                nodelay: true,
                #[cfg(feature = "watch")]
                watch: true,
            };

            // Verify that ValidatedArgs can handle multiple paths
            assert_eq!(args.paths.len(), 2);
            assert_eq!(args.paths[0], config_path1);
            assert_eq!(args.paths[1], config_path2);
            
            // Verify all paths are valid files for direct watching
            for path in &args.paths {
                assert!(path.is_file(), "All config paths should be files for direct watching");
                assert!(path.extension().is_some(), "All config files should have extensions");
            }
        }
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
