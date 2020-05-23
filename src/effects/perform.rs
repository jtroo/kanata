use evdev_rs::enums::EV_KEY;
use crate::keys::KeyCode;
use crate::keys::KeyValue;
use crate::effects::Effect;
use crate::effects::EffectValue;
use crate::kbd_out::KbdOut;
use crate::layers::LayerIndex;
use crate::ktrl::Ktrl;

use log::warn;
use std::io::Error;

fn perform_momentary_layer(ktrl: &mut Ktrl, value: KeyValue, idx: LayerIndex) -> Result<(), Error> {
    if !ktrl.th_mgr.is_idle() {
        warn!("Can't make layer changes while tap-holding");
    } else if value == KeyValue::Press {
        ktrl.l_mgr.turn_layer_on(idx)
    } else if value == KeyValue::Release {
        ktrl.l_mgr.turn_layer_off(idx)
    }

    Ok(())
}

fn perform_toggle_layer(ktrl: &mut Ktrl, value: KeyValue, idx: LayerIndex) -> Result<(), Error> {
    if !ktrl.th_mgr.is_idle() {
        warn!("Can't make layer changes while tap-holding");
    } else if value == KeyValue::Press {
        ktrl.l_mgr.toggle_layer(idx)
    }

    Ok(())
}

fn perform_default(kbd_out: &mut KbdOut, code: KeyCode, value: KeyValue) -> Result<(), Error> {
    let ev_key: EV_KEY = code.into();
    kbd_out.write_key(ev_key, value as i32)
}

pub fn perform_effect(ktrl: &mut Ktrl, fx_val: EffectValue) -> Result<(), Error> {
    match fx_val.fx {
        Effect::Default(code) => perform_default(&mut ktrl.kbd_out, code, fx_val.val),
        Effect::ToggleLayer(idx) => perform_toggle_layer(ktrl, fx_val.val, idx),
        Effect::MomentaryLayer(idx) => perform_momentary_layer(ktrl, fx_val.val, idx),
    }
}
