use anyhow::{anyhow, bail, Result};
use kanata_state_machine::{oskbd::*, *};
use wasm_bindgen::prelude::*;

use std::sync::Once;

static INIT: Once = Once::new();

#[wasm_bindgen]
pub fn init() {
    INIT.call_once(|| {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    });
}

#[wasm_bindgen]
pub fn check_config(cfg: &str) -> JsValue {
    let res = Kanata::new_from_str(cfg);
    JsValue::from_str(&match res {
        Ok(_) => "Config is good!".to_owned(),
        Err(e) => format!("{e:?}"),
    })
}

#[wasm_bindgen]
pub fn simulate(cfg: &str, sim: &str) -> JsValue {
    JsValue::from_str(&match simulate_impl(cfg, sim) {
        Ok(s) => s,
        Err(e) => format!("Config or simulation input has error.\n\n{e:?}"),
    })
}

pub fn simulate_impl(cfg: &str, sim: &str) -> Result<String> {
    let mut k = Kanata::new_from_str(cfg)?;
    let mut accumulated_ticks = 0;
    for l in sim.lines() {
        for pair in l.split_whitespace() {
            match pair.split_once(':') {
                Some((kind, val)) => match kind {
                    "tick" | "🕐" | "t" => {
                        let ticks = str::parse::<u128>(val)
                            .map_err(|e| anyhow!("line: {l}\ninvalid number in {kind}:{val}\n{e}"))?;
                        if ticks > 60000 {
                            bail!("line: {l}\nmax tick is 60000: {kind}:{val}")
                        }
                        for _ in 0..ticks {
                            if !k.can_block_update_idle_waiting(1) {
                                k.tick_ms(1, &None)?;
                            } else {
                                k.kbd_out.tick();
                            }
                        }
                        accumulated_ticks += ticks;
                        if accumulated_ticks > 3600000 {
                            bail!("You are trying to simulate over an hour's worth of time.\nAborting to avoid wasting your CPU cycles.")
                        }
                    }
                    "press" | "↓" | "d" | "down" => {
                        let key_code =
                            str_to_oscode(val).ok_or_else(|| anyhow!("line: {l}\nunknown key in {kind}:{val}"))?;
                        k.handle_input_event(&KeyEvent {
                            code: key_code,
                            value: KeyValue::Press,
                        })?;
                    }
                    "release" | "↑" | "u" | "up" => {
                        let key_code =
                        str_to_oscode(val).ok_or_else(|| anyhow!("line: {l}\nunknown key in {kind}:{val}"))?;
                        k.handle_input_event(&KeyEvent {
                            code: key_code,
                            value: KeyValue::Release,
                        })?;
                    }
                    "repeat" | "⟳" | "r" => {
                        let key_code =
                        str_to_oscode(val).ok_or_else(|| anyhow!("line: {l}\nunknown key in {kind}:{val}"))?;
                        k.handle_input_event(&KeyEvent {
                            code: key_code,
                            value: KeyValue::Repeat,
                        })?;
                    }
                    _ => bail!("line: {l}\ninvalid action: {kind}\nvalid actions:\nu | up\nd | down\nt | tick"),
                },
                None => bail!("line: {l}\ninvalid item: {pair}\nexpected format: action:item"),
            }
        }
    }
    Ok(k.kbd_out
        .outputs
        .events
        .join("\n")
        .replace('↓', "↓(press)   ")
        .replace('↑', "↑(release) "))
}
