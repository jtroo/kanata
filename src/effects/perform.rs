use evdev_rs::enums::EV_KEY;
use crate::keys::KeyCode;
use crate::keys::KeyValue;
use crate::effects::Effect;
use crate::effects::EffectValue;
use crate::kbd_out::KbdOut;
use std::io::Error;

fn perform_default(kbd_out: &mut KbdOut, code: KeyCode, value: KeyValue) -> Result<(), Error> {
    let ev_key: EV_KEY = code.into();
    kbd_out.write_key(ev_key, value as i32)
}

pub fn perform_effect(kbd_out: &mut KbdOut, fx_val: EffectValue) -> Result<(), Error> {
    match fx_val.fx {
        Effect::Default(code) => perform_default(kbd_out, code, fx_val.val),
    }
}
