use clap::{App, Arg};
use log::info;
use simplelog::*;
use std::fs::File;
use std::io::{Error, ErrorKind::*};
use std::path::Path;
use std::sync::mpsc;

mod actions;
mod cfg;
mod effects;
mod kbd_in;
mod kbd_out;
mod keys;
mod ktrl;
mod layers;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use ktrl::Ktrl;
use ktrl::KtrlArgs;

#[cfg(feature = "ipc")]
mod ipc;
#[cfg(feature = "ipc")]
use ipc::KtrlIpc;

const DEFAULT_CFG_PATH: &str = "/opt/ktrl/cfg.ron";
const DEFAULT_LOG_PATH: &str = "/opt/ktrl/log.txt";
const DEFAULT_ASSETS_PATH: &str = "/opt/ktrl/assets";
const DEFAULT_IPC_PORT: &str = "7331";
const DEFAULT_NOTIFY_PORT: &str = "7333";

#[doc(hidden)]
fn cli_init() -> Result<KtrlArgs, std::io::Error> {
    let matches = App::new("ktrl")
        .version("0.1.7")
        .author("Itay G. <thifixp@gmail.com>")
        .about("Unleashes your keyboard's full potential")
        .arg(
            Arg::with_name("device")
                .short("d")
                .long("device")
                .value_name("DEVICE")
                .help("Path to your keyboard's input device. Usually in /dev/input/")
                .takes_value(true)
                .required(true),
        )
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
            Arg::with_name("assets")
                .long("assets")
                .value_name("ASSETS")
                .help(&format!(
                    "Path ktrl's assets directory. Default: {}",
                    DEFAULT_ASSETS_PATH
                ))
                .takes_value(true),
        )
        .arg(
            Arg::with_name("logfile")
                .long("log")
                .value_name("LOGFILE")
                .help(&format!(
                    "Path to the log file. Default: {}",
                    DEFAULT_LOG_PATH
                ))
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ipc_port")
                .long("ipc-port")
                .value_name("IPC-PORT")
                .help(&format!(
                    "TCP Port to listen on for ipc requests. Default: {}",
                    DEFAULT_IPC_PORT
                ))
                .takes_value(true),
        )
        .arg(
            Arg::with_name("notify_port")
                .long("notify-port")
                .value_name("NOTIFY-PORT")
                .help(&format!(
                    "TCP Port where notifications will be sent. Default: {}",
                    DEFAULT_NOTIFY_PORT
                ))
                .takes_value(true),
        )
        .arg(
            Arg::with_name("msg")
                .long("msg")
                .value_name("IPC-MSG")
                .help("IPC Message to the running ktrl daemon. Won't start a new ktrl instance")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Enables debug level logging"),
        )
        .get_matches();

    let config_path = Path::new(matches.value_of("cfg").unwrap_or(DEFAULT_CFG_PATH));
    let log_path = Path::new(matches.value_of("logfile").unwrap_or(DEFAULT_LOG_PATH));
    let assets_path = Path::new(matches.value_of("assets").unwrap_or(DEFAULT_ASSETS_PATH));
    let kbd_path = Path::new(matches.value_of("device").unwrap());
    let ipc_port = matches
        .value_of("ipc_port")
        .unwrap_or(DEFAULT_IPC_PORT)
        .parse::<usize>()
        .expect("Bad ipc port value");
    let ipc_msg = matches.value_of("msg").map(|x: &str| x.to_string());
    let notify_port = matches
        .value_of("notify_port")
        .unwrap_or(DEFAULT_NOTIFY_PORT)
        .parse::<usize>()
        .expect("Bad notify port value");

    let log_lvl = match matches.is_present("debug") {
        true => LevelFilter::Debug,
        _ => LevelFilter::Info,
    };

    CombinedLogger::init(vec![
        TermLogger::new(log_lvl, Config::default(), TerminalMode::Mixed),
        WriteLogger::new(
            log_lvl,
            Config::default(),
            File::create(log_path).expect("Couldn't initialize the file logger"),
        ),
    ])
    .expect("Couldn't initialize the logger");

    if !config_path.exists() {
        let err = format!(
            "Could not find your config file ({})",
            config_path.to_str().unwrap_or("?")
        );
        return Err(Error::new(NotFound, err));
    }

    if !kbd_path.exists() {
        let err = format!(
            "Could not find the keyboard device ({})",
            kbd_path.to_str().unwrap_or("?")
        );
        return Err(Error::new(NotFound, err));
    }

    Ok(KtrlArgs {
        kbd_path: kbd_path.to_path_buf(),
        config_path: config_path.to_path_buf(),
        assets_path: assets_path.to_path_buf(),
        ipc_port,
        ipc_msg,
        notify_port,
    })
}

#[cfg(feature = "ipc")]
fn main_impl(args: KtrlArgs) -> Result<(), std::io::Error> {
    let ipc_port = args.ipc_port;

    // Operate as a client, then quit
    if let Some(ipc_msg) = args.ipc_msg {
        return KtrlIpc::send_ipc_req(ipc_port, ipc_msg);
    }

    // Otherwise, startup the server
    let ktrl_arc = Ktrl::new_arc(args)?;
    info!("ktrl: Setup Complete");

    let ipc = KtrlIpc::new(ktrl_arc.clone(), ipc_port)?;
    ipc.spawn_ipc_thread();

    // Start a processing loop in another thread and run the event loop in this thread.
    //
    // The reason for two different event loops is that the "event_loop" only listens for keyboard
    // events, which it sends to the "processing loop". The processing loop handles keyboard events
    // as well as time-based events such as a tap-hold hold duration expiring.
    let (tx, rx) = mpsc::channel();
    Ktrl::start_processing_loop(ktrl_arc.clone(), rx);
    Ktrl::event_loop(ktrl_arc)?;

    Ok(())
}

#[cfg(not(feature = "ipc"))]
fn main_impl(args: KtrlArgs) -> Result<(), std::io::Error> {
    let ktrl_arc = Ktrl::new_arc(args)?;
    info!("ktrl: Setup Complete");

    // See above for explanation of the two loops.
    let (tx, rx) = mpsc::channel();
    Ktrl::start_processing_loop(ktrl_arc.clone(), rx);
    Ktrl::event_loop(ktrl_arc, tx)?;

    Ok(())
}

#[doc(hidden)]
fn main() -> Result<(), std::io::Error> {
    let args = cli_init()?;
    main_impl(args)
}
