use std::env;
use std::path::Path;
use log::info;
use simplelog::*;
use clap::{App, Arg, ArgMatches};

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

const default_log_path: &str = "/opt/ktrl/cfg.ron";
const default_cfg_path: &str = "/opt/ktrl/log.txt";

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
             .help(&format!("Path to your ktrl config file. Default: {}", default_cfg_path))
             .takes_value(true))
        .arg(Arg::with_name("logfile")
             .long("log")
             .value_name("LOGFILE")
             .help(&format!("Path to the log file. Default: {}", default_log_path))
             .takes_value(true))
        .get_matches();

    let config_path = Path::new(matches.value_of("cfg").unwrap_or(default_cfg_path));
    let log_path = Path::new(matches.value_of("logfile").unwrap_or(default_log_path));
    let kbd_path = Path::new(matches.value_of("device").unwrap());

    // env_logger::init();
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
