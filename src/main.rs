use std::env;
use std::path::Path;
use log::info;

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

fn main() -> Result<(), std::io::Error> {
    let kbd_path_str = env::args().nth(1).expect("Missing keyboard path");

    env_logger::init();
    let kbd_path = Path::new(&kbd_path_str);
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
