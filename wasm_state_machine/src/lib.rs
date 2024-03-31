use kanata_state_machine::{*, oskbd::*};
use wasm_bindgen::prelude::*;
use anyhow::{anyhow, bail, Result};

use std::sync::Once;

static INIT: Once = Once::new();

#[wasm_bindgen]
pub fn init() {
    INIT.call_once(|| {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        wasm_logger::init(wasm_logger::Config::default());
    });
}

#[wasm_bindgen]
pub fn check_config(cfg: &str) -> JsValue {
    let res = Kanata::new_from_str(cfg);
    log::info!("hi out of kanata");
    JsValue::from_str(&match res {
        Ok(_) => "Config is good!".to_owned(),
        Err(e) => format!("{e:?}"),
    })
}

#[wasm_bindgen]
pub fn simulate(cfg: &str, sim: &str)-> JsValue {
    JsValue::from_str(&match simulate_impl(cfg, sim) {
        Ok(s) => s,
        Err(e) => format!("Config or simulation input has error.\n\n{e:?}"),
    })
}

pub fn simulate_impl(cfg: &str, sim: &str) -> Result<String> {
    let mut k = Kanata::new_from_str(cfg)?;
    for l in sim.lines() {
        for pair in l.split_whitespace() {
            match pair.split_once(':') {
                Some((kind, val)) => match kind {
                    "tick" | "🕐" | "t" => {
                        let tick = str::parse::<u128>(val)?;
                        k.tick_ms(tick, &None)?;
                    }
                    "press" | "↓" | "d" | "down" => {
                        let key_code =
                            str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?;
                        k.handle_input_event(&KeyEvent {
                            code: key_code,
                            value: KeyValue::Press,
                        })?;
                    }
                    "release" | "↑" | "u" | "up" => {
                        let key_code =
                            str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?;
                        k.handle_input_event(&KeyEvent {
                            code: key_code,
                            value: KeyValue::Release,
                        })?;
                    }
                    "repeat" | "⟳" | "r" => {
                        let key_code =
                            str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?;
                        k.handle_input_event(&KeyEvent {
                            code: key_code,
                            value: KeyValue::Repeat,
                        })?;
                    }
                    _ => bail!("invalid pair prefix: {kind}"),
                },
                None => bail!("invalid pair: {l}"),
            }
        }
    }
    Ok(k.kbd_out.outputs.join("\n"))
}
