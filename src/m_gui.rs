#![allow(unused_imports,unused_variables,unreachable_code,dead_code,non_upper_case_globals)]
// #![allow(non_upper_case_globals)]

extern crate native_windows_gui    as nwg;
extern crate native_windows_derive as nwd;
use nwd::NwgUi;
use nwg::NativeUi;

#[derive(Default, NwgUi)] pub struct SystemTray {
  #[nwg_control]                                           	window	: nwg::MessageWindow,
  #[nwg_resource(source_file:Some("../assets/kanata.ico"))]	icon  	: nwg::Icon,
  #[nwg_control(icon:Some(&data.icon), tip: Some("Hello"))]	//
   #[nwg_events(MousePressLeftUp:[SystemTray::show_menu]   	//
    ,           OnContextMenu   :[SystemTray::show_menu])] 	tray     	: nwg::TrayNotification,
  #[nwg_control(parent:window   , popup: true)]            	tray_menu	: nwg::Menu,
  #[nwg_control(parent:tray_menu, text:"&1 Hello")]        	//
   #[nwg_events(OnMenuItemSelected:[SystemTray::hello1])]  	tray_item1	: nwg::MenuItem,
  #[nwg_control(parent:tray_menu, text:"&2 Popup")]        	//
   #[nwg_events(OnMenuItemSelected:[SystemTray::hello2])]  	tray_item2	: nwg::MenuItem,
  #[nwg_control(parent:tray_menu, text:"&X Exit")]         	//
   #[nwg_events(OnMenuItemSelected:[SystemTray::exit  ])]  	tray_item3	: nwg::MenuItem,
}
impl SystemTray {
  fn show_menu(&self) {
    let (x, y) = nwg::GlobalCursor::position();
    self.tray_menu.popup(x, y);  }
  fn hello1(&self) {nwg::simple_message("Hello", "Hello World!");}
  fn hello2(&self) {
    let flags = nwg::TrayNotificationFlags::USER_ICON | nwg::TrayNotificationFlags::LARGE_ICON;
    self.tray.show("Hello World", Some("Welcome to my application"), Some(flags), Some(&self.icon));  }
  fn exit(&self) {nwg::stop_thread_dispatch();}
}




use anyhow::{bail, Result};
use clap::Parser;
use kanata_parser::cfg;
use crate::*;
use log::info;
use simplelog::*;

use std::path::PathBuf;

// #[cfg(test)]
// mod tests;

#[derive(Parser, Debug)]
// #[command(author, version, verbatim_doc_comment)]
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

  /// Enable debug logging.
  #[arg(short, long)]
  debug: bool,

  /// Enable trace logging; implies --debug as well.
  #[arg(short, long)]
  trace: bool,

  /// Remove the startup delay on kanata.
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

  let log_lvl = match (args.debug, args.trace) {
    (_, true) => LevelFilter::Trace,
    (true, false) => LevelFilter::Debug,
    (false, false) => LevelFilter::Info,
  };

  let mut log_cfg = ConfigBuilder::new();
  if let Err(e) = log_cfg.set_time_offset_to_local() {
    eprintln!("WARNING: could not set log TZ to local: {e:?}");
  };
  log_cfg.set_time_format_rfc3339();
  // todo: use this logger with WinDbg
  if *IS_TERM	{
    CombinedLogger::init(vec![
    TermLogger ::new(log_lvl,log_cfg.build(),TerminalMode::Mixed,ColorChoice::AlwaysAnsi,),
    WriteLogger::new(log_lvl,log_cfg.build(),log_win::WINDBG_LOGGER),
    ]).expect("logger can init");
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
    let args = cli_init()?; // parse CLI arguments and initialize logging
    #[cfg(not(feature = "passthru_ahk"))]
    let cfg_arc = Kanata::new_arc(&args)?; // new configuration from a file
    #[cfg(feature = "passthru_ahk")]
    let cfg_arc = Kanata::new_arc(&args, None)?; // new configuration from a file
    if !args.nodelay {
        info!("Sleeping for 2s. Please release all keys and don't press additional ones. Run kanata with --help to see how understand more and how to disable this sleep.");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Start a processing loop in another thread and run the event loop in this thread.
    // The reason for two different event loops is that the "event loop" only listens for keyboard events, which it sends to the "processing loop". The processing loop handles keyboard events while also maintaining `tick()` calls to keyberon.
    let (tx, rx) = std::sync::mpsc::sync_channel(100);

    let (server, ntx, nrx) = if let Some(address) = {
      #[cfg(feature = "tcp_server")]{args.tcp_server_address}
      #[cfg(not(feature = "tcp_server"))]{None::<SocketAddrWrapper>}
    } {
        let mut server = TcpServer::new(address.into_inner(), tx.clone());
        server.start(cfg_arc.clone());
        let (ntx, nrx) = std::sync::mpsc::sync_channel(100);
        (Some(server), Some(ntx), Some(nrx))
    } else {
        (None, None, None)
    };

    Kanata::start_processing_loop(cfg_arc.clone(), rx, ntx, args.nodelay); // 2 handles keyboard events while also maintaining `tick()` calls to keyberon

    if let (Some(server), Some(nrx)) = (server, nrx) {
        #[allow(clippy::unit_arg)]
        Kanata::start_notification_loop(nrx, server.connections);
    }
    #[cfg(target_os = "linux")]
    sd_notify::notify(true, &[sd_notify::NotifyState::Ready])?;

    Kanata::event_loop(cfg_arc, tx)?; // 1 only listens for keyboard events

    Ok(())
}

use log::*;
use win_dbg_logger as log_win;
fn log_init(max_lvl: &i8) {
    let _ = log_win::init();
    let a = log_win::set_thread_state(true);
    let log_lvl = match max_lvl {
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    };
    log::set_max_level(log_lvl);
}

use once_cell::sync::Lazy;
static IS_TERM:Lazy<bool> = Lazy::new(||stdout().is_terminal());

use winapi::um::wincon::{AttachConsole, FreeConsole, ATTACH_PARENT_PROCESS};
use winapi::shared::minwindef::BOOL;
use std::io::{stdout, IsTerminal};
pub fn main_gui() {
  let is_attached:BOOL; // doesn't attach in GUI launch mode
  unsafe {is_attached = AttachConsole(ATTACH_PARENT_PROCESS);};
  if *IS_TERM	{
    println!("println terminal; is_attached console = {:?}",is_attached); // GUI launch will have no console
    log::info!("log::info terminal; is_attached console = {:?}",is_attached); // isn't ready yet
  } else {
    log_init(&4);
    info!("I'm not a terminal");
  }
  let ret = main_impl();
  if let Err(ref e) = ret {log::error!("{e}\n");}
  unsafe {FreeConsole();}
}
