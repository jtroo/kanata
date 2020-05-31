use crate::effects::Effect;
use crate::effects::EffectValue;
use crate::effects::KSnd;
use crate::kbd_out::KbdOut;
use crate::keys::KeyCode;
use crate::keys::KeyCode::*;
use crate::keys::KeyValue;
use crate::ktrl::Ktrl;
use crate::layers::LayerIndex;

use std::io::Error;
use std::vec::Vec;

lazy_static::lazy_static! {
    static ref HYPER: Vec<KeyCode> = {
        vec![
            KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT, KEY_LEFTMETA
        ].iter()
            .map(|evkey| KeyCode::from(evkey.clone()))
            .collect()
    };
}

lazy_static::lazy_static! {
    static ref MEH: Vec<KeyCode> = {
        vec![
            KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT
        ].iter()
            .map(|evkey| KeyCode::from(evkey.clone()))
            .collect()
    };
}

fn perform_multiple_effects(
    ktrl: &mut Ktrl,
    effects: Vec<Effect>,
    value: KeyValue,
) -> Result<(), Error> {
    for fx in effects {
        let sub_fx_val = EffectValue::new(fx.clone(), value);
        perform_effect(ktrl, sub_fx_val)?;
    }

    Ok(())
}

fn perform_play_custom_sound(
    ktrl: &mut Ktrl,
    snd_path: String,
    value: KeyValue,
) -> Result<(), Error> {
    if value == KeyValue::Press {
        ktrl.dj.play_custom(&snd_path)
    }

    Ok(())
}

fn perform_play_sound(ktrl: &mut Ktrl, snd: KSnd, value: KeyValue) -> Result<(), Error> {
    if value == KeyValue::Press {
        ktrl.dj.play(snd)
    }

    Ok(())
}

fn perform_momentary_layer(ktrl: &mut Ktrl, idx: LayerIndex, value: KeyValue) -> Result<(), Error> {
    if value == KeyValue::Press {
        ktrl.l_mgr.turn_layer_on(idx)
    } else if value == KeyValue::Release {
        ktrl.l_mgr.turn_layer_off(idx)
    }

    Ok(())
}

fn perform_toggle_layer(ktrl: &mut Ktrl, idx: LayerIndex, value: KeyValue) -> Result<(), Error> {
    if value == KeyValue::Press {
        ktrl.l_mgr.toggle_layer(idx)
    }

    Ok(())
}

fn perform_toggle_layer_alias(ktrl: &mut Ktrl, name: String, value: KeyValue) -> Result<(), Error> {
    if value == KeyValue::Press {
        ktrl.l_mgr.toggle_layer_alias(name)
    }

    Ok(())
}

fn perform_key_sticky(ktrl: &mut Ktrl, code: KeyCode, value: KeyValue) -> Result<(), Error> {
    if value == KeyValue::Release {
        return Ok(());
    }

    if !ktrl.sticky.is_pressed(code) {
        ktrl.sticky.update_pressed(&mut ktrl.l_mgr, code);
        ktrl.kbd_out.press_key(code)
    } else {
        ktrl.sticky.update_released(&mut ktrl.l_mgr, code);
        ktrl.kbd_out.release_key(code)
    }
}

fn perform_keyseq(kbd_out: &mut KbdOut, seq: Vec<KeyCode>, value: KeyValue) -> Result<(), Error> {
    for code in seq {
        perform_key(kbd_out, code, value)?;
    }

    Ok(())
}

fn perform_key(kbd_out: &mut KbdOut, code: KeyCode, value: KeyValue) -> Result<(), Error> {
    kbd_out.write_key(code, value)
}

pub fn perform_effect(ktrl: &mut Ktrl, fx_val: EffectValue) -> Result<(), Error> {
    match fx_val.fx {
        Effect::NoOp => Ok(()),
        Effect::Key(code) => perform_key(&mut ktrl.kbd_out, code, fx_val.val),
        Effect::KeySeq(seq) => perform_keyseq(&mut ktrl.kbd_out, seq, fx_val.val),
        Effect::KeySticky(code) => perform_key_sticky(ktrl, code, fx_val.val),
        Effect::Meh => perform_keyseq(&mut ktrl.kbd_out, MEH.to_vec(), fx_val.val),
        Effect::Hyper => perform_keyseq(&mut ktrl.kbd_out, HYPER.to_vec(), fx_val.val),
        Effect::ToggleLayer(idx) => perform_toggle_layer(ktrl, idx, fx_val.val),
        Effect::ToggleLayerAlias(name) => perform_toggle_layer_alias(ktrl, name, fx_val.val),
        Effect::MomentaryLayer(idx) => perform_momentary_layer(ktrl, idx, fx_val.val),
        Effect::Sound(snd) => perform_play_sound(ktrl, snd, fx_val.val),
        Effect::SoundEx(snd) => perform_play_custom_sound(ktrl, snd, fx_val.val),
        Effect::Multi(fxs) => perform_multiple_effects(ktrl, fxs, fx_val.val),
    }
}
