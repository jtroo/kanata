use std::env;
use std::path::Path;

mod kbd_in;
mod kbd_out;
mod ktrl;
mod layers;
mod keycode;
mod keyevent;
mod actions;
mod effects;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use layers::LayersManager;
use ktrl::Ktrl;
use actions::TapHoldMgr;

fn main() -> Result<(), std::io::Error> {
    let kbd_path_str = env::args().nth(1).expect("Missing keyboard path");

    let kbd_path = Path::new(&kbd_path_str);
    let kbd_in = KbdIn::new(kbd_path)?;
    let l_mgr = LayersManager::new(vec![]);
    let th_mgr = TapHoldMgr::new();

    let kbd_out = KbdOut::new()?;
    println!("ktrl: Setup Complete");

    let mut ktrl = Ktrl::new(kbd_in, kbd_out, l_mgr, th_mgr);
    ktrl.event_loop()?;

    Ok(())
}
