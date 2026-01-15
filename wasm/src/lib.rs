use anyhow::{anyhow, bail, Result};
use kanata_state_machine::{kanata::handle_fakekey_action, oskbd::*, *};
use rustc_hash::FxHashMap;
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
    let (cfg, files) = split_cfg_and_sim_files(cfg);
    let res = Kanata::new_from_str(&cfg, files);
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

fn split_cfg_and_sim_files(original_cfg: &str) -> (String, FxHashMap<String, String>) {
    let mut cfg = String::new();
    let mut file_name = None;
    let mut file = String::new();
    let mut sim_files = Default::default();

    let mut original_lines = original_cfg.lines();
    const FILE_PREFIX: &str = "=== file:";

    // Parse main configuration.
    // Must not consume whole iterator here.
    #[allow(clippy::while_let_on_iterator)]
    while let Some(line) = original_lines.next() {
        if line.starts_with(FILE_PREFIX) {
            file_name = line.strip_prefix(FILE_PREFIX);
            break;
        }
        cfg.push_str(line);
        cfg.push('\n');
    }
    if file_name.is_none() {
        return (cfg, sim_files);
    }

    // Parse simulated sim_files.
    for line in original_lines {
        if line.starts_with(FILE_PREFIX) {
            sim_files.insert(file_name.unwrap().to_string(), file.clone());
            file_name = line.strip_prefix(FILE_PREFIX);
            file.clear();
            continue;
        }
        file.push_str(line);
        file.push('\n');
    }
    // Save the last file
    sim_files.insert(file_name.unwrap().to_string(), file.clone());
    (cfg, sim_files)
}

fn parse_fakekey_spec(spec: &str) -> Result<(&str, FakeKeyAction)> {
    let (name, action) = match spec.split_once(':') {
        Some((name, action_str)) => {
            let action = match action_str {
                "press" | "p" => FakeKeyAction::Press,
                "release" => FakeKeyAction::Release,
                "tap" | "t" => FakeKeyAction::Tap,
                "toggle" | "g" => FakeKeyAction::Toggle,
                _ => bail!(
                    "unknown fakekey action: {action_str}. Expected: press, release, tap, or toggle"
                ),
            };
            (name, action)
        }
        None => (spec, FakeKeyAction::Press),
    };
    if name.is_empty() {
        bail!("fakekey name cannot be empty");
    }
    Ok((name, action))
}

fn apply_fakekey_action(k: &mut Kanata, name: &str, action: FakeKeyAction) -> Result<()> {
    let index = k
        .virtual_keys
        .get(name)
        .ok_or_else(|| anyhow!("unknown virtual key: {name}"))?;
    handle_fakekey_action(action, k.layout.bm(), FAKE_KEY_ROW, *index as u16);
    Ok(())
}

fn simulate_impl(cfg: &str, sim: &str) -> Result<String> {
    let (cfg, files) = split_cfg_and_sim_files(cfg);
    let mut k = Kanata::new_from_str(&cfg, files)?;
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
                    // Virtual/fake key activation: vk:name[:action]
                    "vk" | "fakekey" | "virtualkey" | "🎭" => {
                        let (vk_name, action) = parse_fakekey_spec(val)?;
                        apply_fakekey_action(&mut k, vk_name, action)?;
                    }
                    // Layer switch: ls:layer_name
                    "ls" | "layer-switch" | "🔀" => {
                        let layer_idx = k
                            .layer_info
                            .iter()
                            .position(|l| l.name == val)
                            .ok_or_else(|| anyhow!("line: {l}\nunknown layer: {val}"))?;
                        k.layout.bm().set_default_layer(layer_idx);
                    }
                    _ => bail!("line: {l}\ninvalid action: {kind}\nvalid actions:\nu | up\nd | down\nt | tick\nvk | fakekey\nls | layer-switch"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sim(cfg: &str, sim: &str) -> String {
        simulate_impl(cfg, sim).expect("simulation should succeed")
    }

    fn sim_err(cfg: &str, sim: &str) -> String {
        simulate_impl(cfg, sim).expect_err("simulation should fail").to_string()
    }

    #[test]
    fn basic_key_press_release() {
        let result = sim(
            "(defsrc a)(deflayer base b)",
            "d:a t:10 u:a t:1",
        );
        assert!(result.contains("↓(press)   B"));
        assert!(result.contains("↑(release) B"));
    }

    #[test]
    fn tick_syntax_variants() {
        let result = sim(
            "(defsrc a)(deflayer base b)",
            "d:a t:5 u:a tick:5 d:a 🕐:5 u:a t:1",
        );
        assert!(result.contains("t:5ms"));
    }

    #[test]
    fn press_syntax_variants() {
        let result = sim(
            "(defsrc a b c d)(deflayer base 1 2 3 4)",
            "press:a t:1 u:a t:1 down:b t:1 u:b t:1 ↓:c t:1 u:c t:1 d:d t:1 u:d t:1",
        );
        assert!(result.contains("↓(press)   Kb1"));
        assert!(result.contains("↓(press)   Kb2"));
        assert!(result.contains("↓(press)   Kb3"));
        assert!(result.contains("↓(press)   Kb4"));
    }

    #[test]
    fn release_syntax_variants() {
        let result = sim(
            "(defsrc a b c d)(deflayer base 1 2 3 4)",
            "d:a t:1 release:a t:1 d:b t:1 up:b t:1 d:c t:1 ↑:c t:1 d:d t:1 u:d t:1",
        );
        assert!(result.contains("↑(release) Kb1"));
        assert!(result.contains("↑(release) Kb2"));
        assert!(result.contains("↑(release) Kb3"));
        assert!(result.contains("↑(release) Kb4"));
    }

    #[test]
    fn repeat_key() {
        let result = sim(
            "(defsrc a)(deflayer base b)",
            "d:a t:10 r:a t:10 u:a t:1",
        );
        assert!(result.contains("↓(press)   B"));
        assert!(result.contains("↑(release) B"));
    }

    #[test]
    fn layer_switch() {
        let result = sim(
            "(defsrc a)(deflayer base a)(deflayer other 1)",
            "ls:other d:a t:10 u:a t:1",
        );
        assert!(result.contains("↓(press)   Kb1"));
    }

    #[test]
    fn layer_switch_back_to_base() {
        let result = sim(
            "(defsrc a)(deflayer base a)(deflayer other 1)",
            "ls:other d:a t:10 u:a t:1 ls:base d:a t:10 u:a t:1",
        );
        assert!(result.contains("↓(press)   Kb1"));
        assert!(result.contains("↓(press)   A"));
    }

    #[test]
    fn layer_switch_syntax_variants() {
        let r1 = sim("(defsrc a)(deflayer base a)(deflayer x 1)", "ls:x d:a t:1 u:a t:1");
        let r2 = sim("(defsrc a)(deflayer base a)(deflayer x 1)", "layer-switch:x d:a t:1 u:a t:1");
        let r3 = sim("(defsrc a)(deflayer base a)(deflayer x 1)", "🔀:x d:a t:1 u:a t:1");
        assert!(r1.contains("Kb1"));
        assert!(r2.contains("Kb1"));
        assert!(r3.contains("Kb1"));
    }

    #[test]
    fn virtual_key_press() {
        let result = sim(
            "(defsrc a)(defvirtualkeys vk1 lctl)(deflayer base a)",
            "vk:vk1 t:10",
        );
        assert!(result.contains("↓(press)   LCtrl"));
    }

    #[test]
    fn virtual_key_tap() {
        let result = sim(
            "(defsrc a)(defvirtualkeys vk1 lctl)(deflayer base a)",
            "vk:vk1:tap t:10",
        );
        assert!(result.contains("↓(press)   LCtrl"));
        assert!(result.contains("↑(release) LCtrl"));
    }

    #[test]
    fn virtual_key_syntax_variants() {
        let r1 = sim("(defsrc a)(defvirtualkeys v lctl)(deflayer base a)", "vk:v t:1");
        let r2 = sim("(defsrc a)(defvirtualkeys v lctl)(deflayer base a)", "fakekey:v t:1");
        let r3 = sim("(defsrc a)(defvirtualkeys v lctl)(deflayer base a)", "virtualkey:v t:1");
        let r4 = sim("(defsrc a)(defvirtualkeys v lctl)(deflayer base a)", "🎭:v t:1");
        assert!(r1.contains("LCtrl"));
        assert!(r2.contains("LCtrl"));
        assert!(r3.contains("LCtrl"));
        assert!(r4.contains("LCtrl"));
    }

    #[test]
    fn error_unknown_key() {
        let err = sim_err("(defsrc a)(deflayer base a)", "d:notakey");
        assert!(err.contains("unknown key"));
    }

    #[test]
    fn error_unknown_layer() {
        let err = sim_err("(defsrc a)(deflayer base a)", "ls:notalayer");
        assert!(err.contains("unknown layer"));
    }

    #[test]
    fn error_unknown_virtual_key() {
        let err = sim_err("(defsrc a)(deflayer base a)", "vk:notavk");
        assert!(err.contains("unknown virtual key"));
    }

    #[test]
    fn error_invalid_action() {
        let err = sim_err("(defsrc a)(deflayer base a)", "badaction:a");
        assert!(err.contains("invalid action"));
    }

    #[test]
    fn error_missing_colon() {
        let err = sim_err("(defsrc a)(deflayer base a)", "nocolon");
        assert!(err.contains("expected format"));
    }
}
