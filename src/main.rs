use clap::{App, Arg};
use log::info;
use simplelog::*;
use std::fs::File;
use std::io::{Error, ErrorKind::*};
use std::path::Path;
use std::sync::mpsc;

mod cfg;
mod kbd_in;
mod kbd_out;
mod keys;
mod ktrl;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use ktrl::Ktrl;
use ktrl::KtrlArgs;

const DEFAULT_CFG_PATH: &str = "./ktrl.kbd";
const DEFAULT_LOG_PATH: &str = "/tmp/ktrl-log.txt";

fn cli_init() -> Result<KtrlArgs, std::io::Error> {
    let matches = App::new("ktrl")
        .version("0.0.1")
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
            Arg::with_name("debug")
                .long("debug")
                .help("Enables debug level logging"),
        )
        .get_matches();

    let config_path = Path::new(matches.value_of("cfg").unwrap_or(DEFAULT_CFG_PATH));
    let log_path = Path::new(matches.value_of("logfile").unwrap_or(DEFAULT_LOG_PATH));
    let kbd_path = Path::new(matches.value_of("device").unwrap());

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
    })
}

fn main_impl(args: KtrlArgs) -> Result<(), std::io::Error> {
    let ktrl_arc = Ktrl::new_arc(args)?;
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

fn main() -> Result<(), std::io::Error> {
    let args = cli_init()?;
    main_impl(args)
}
