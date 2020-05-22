use std::env;
use std::path::Path;

mod kbd_in;
mod kbd_out;
mod ktrl;
mod layers;
mod keys;
mod actions;
mod effects;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use layers::Layers;
use layers::LayersManager;
use ktrl::Ktrl;
use actions::TapHoldMgr;

// ----------------------------------------------
// ---------------- REMOVE ----------------------
// ----------------------------------------------

use evdev_rs::enums::EV_KEY;
use evdev_rs::enums::EV_KEY::*;
use keys::KeyCode;
use layers::Action;
use layers::Effect;

fn make_taphold_action(tap: EV_KEY, hold: EV_KEY) -> Action {
    let tap_fx = Effect::Default(tap.into());
    let hold_fx = Effect::Default(hold.into());
    Action::TapHold(tap_fx, hold_fx)
}

fn make_taphold_layer_entry(src: EV_KEY, tap: EV_KEY, hold: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = make_taphold_action(tap, hold);
    return (src_code, action)
}

fn my_layers() -> Layers {
    vec![
        // 0: base layer
        [
            make_taphold_layer_entry(KEY_A, KEY_A, KEY_LEFTSHIFT),
            make_taphold_layer_entry(KEY_S, KEY_S, KEY_LEFTALT),
        ].iter().cloned().collect(),
    ]
}

// ----------------------------------------------

fn main() -> Result<(), std::io::Error> {
    let kbd_path_str = env::args().nth(1).expect("Missing keyboard path");

    let kbd_path = Path::new(&kbd_path_str);
    let kbd_in = KbdIn::new(kbd_path)?;
    let kbd_out = KbdOut::new()?;

    let mut l_mgr = LayersManager::new(my_layers());
    l_mgr.init();

    let th_mgr = TapHoldMgr::new();
    println!("ktrl: Setup Complete");

    let mut ktrl = Ktrl::new(kbd_in, kbd_out, l_mgr, th_mgr);
    ktrl.event_loop()?;

    Ok(())
}
