use kanata_state_machine::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn check_config(cfg: &str) -> JsValue {
    let res = Kanata::new_from_str(cfg);
    JsValue::from_str(&match res {
        Ok(_) => "Config is good!".to_owned(),
        Err(e) => format!("Config has error\n{e:?}"),
    })
}
