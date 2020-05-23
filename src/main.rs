use std::path::Path;
use log::{info, error};
use simplelog::*;
use clap::{App, Arg};
use nix::unistd::Uid;
use std::io::{Error, ErrorKind::*};
use std::fs::File;

mod kbd_in;
mod kbd_out;
mod ktrl;
mod layers;
mod keys;
mod actions;
mod effects;
mod cfg;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use layers::LayersManager;
use ktrl::Ktrl;
use actions::TapHoldMgr;
use effects::StickyState;

const DEFAULT_CFG_PATH: &str = "/opt/ktrl/cfg.ron";
const DEFAULT_LOG_PATH: &str = "/opt/ktrl/log.txt";

fn is_root() -> bool {
    Uid::effective().is_root()
}

fn main() -> Result<(), std::io::Error> {
    let matches =
        App::new("ktrl")
        .version("0.1")
        .author("Itay G. <thifixp@gmail.com>")
        .about("Unleashes your keyboard's full potential")
        .arg(Arg::with_name("device")
             .short("d")
             .long("device")
             .value_name("DEVICE")
             .help("Path to your keyboard's input device. Usually in /dev/input/")
             .takes_value(true)
             .required(true))
        .arg(Arg::with_name("cfg")
             .long("cfg")
             .value_name("CONFIG")
             .help(&format!("Path to your ktrl config file. Default: {}", DEFAULT_CFG_PATH))
             .takes_value(true))
        .arg(Arg::with_name("logfile")
             .long("log")
             .value_name("LOGFILE")
             .help(&format!("Path to the log file. Default: {}", DEFAULT_LOG_PATH))
             .takes_value(true))
        .arg(Arg::with_name("debug")
             .long("debug")
             .help("Enables debug level logging"))
        .get_matches();

    let config_path = Path::new(matches.value_of("cfg").unwrap_or(DEFAULT_CFG_PATH));
    let log_path = Path::new(matches.value_of("logfile").unwrap_or(DEFAULT_LOG_PATH));
    let kbd_path = Path::new(matches.value_of("device").unwrap());

    if !is_root() {
        return Err(Error::new(PermissionDenied, "Please re-run ktrl as root"));
    }

    let log_lvl = match matches.is_present("debug") {
        true => LevelFilter::Debug,
        _ => LevelFilter::Info,
    };

    CombinedLogger::init(
        vec![
            TermLogger::new(log_lvl, Config::default(), TerminalMode::Mixed),
            WriteLogger::new(log_lvl, Config::default(), File::create(log_path)
                             .expect("Couldn't initialize the file logger")),
        ]
    ).expect("Couldn't initialize the logger");

    if !config_path.exists() {
        let err =  format!("Could not find your config file ({})",
                           config_path.to_str().unwrap_or("?"));
        return Err(Error::new(NotFound, err));
    }

    if !kbd_path.exists() {
        let err =  format!("Could not find the keyboard device ({})",
                           kbd_path.to_str().unwrap_or("?"));
        return Err(Error::new(NotFound, err));
    }

    let kbd_in = KbdIn::new(kbd_path)?;
    let kbd_out = KbdOut::new()?;

    let mut l_mgr = LayersManager::new(cfg::my_layers());
    l_mgr.init();

    let th_mgr = TapHoldMgr::new();
    let sticky = StickyState::new();
    info!("ktrl: Setup Complete");

    let mut ktrl = Ktrl{kbd_in, kbd_out, l_mgr, th_mgr, sticky};
    ktrl.event_loop()?;

    Ok(())
}
