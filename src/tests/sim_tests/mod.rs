//! Contains tests that use simulated inputs.
//!
//! One way to write tests is to write the configuration, write the simulated input, and then let
//! the test fail by comparing the output to an empty string. Run the test then inspect the failure
//! and see if the real output looks sensible according to what is expected.

use crate::tests::*;
use crate::{
    Kanata,
    oskbd::{KeyEvent, KeyValue},
    str_to_oscode,
};

use rustc_hash::FxHashMap;

mod block_keys_tests;
mod capsword_sim_tests;
mod chord_sim_tests;
mod delay_tests;
mod layer_sim_tests;
mod macro_sim_tests;
mod oneshot_tests;
mod output_chord_tests;
mod override_tests;
mod release_sim_tests;
mod repeat_sim_tests;
mod seq_sim_tests;
mod switch_sim_tests;
mod tap_hold_tests;
mod template_sim_tests;
mod timing_tests;
mod unicode_sim_tests;
mod unmod_sim_tests;
mod use_defsrc_sim_tests;
mod vkey_sim_tests;
#[cfg(feature = "zippychord")]
mod zippychord_sim_tests;

fn simulate<S: AsRef<str>>(cfg: S, sim: S) -> String {
    simulate_with_file_content(cfg, sim, Default::default())
}

fn simulate_with_file_content<S: AsRef<str>>(
    cfg: S,
    sim: S,
    file_content: FxHashMap<String, String>,
) -> String {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut k = Kanata::new_from_str(cfg.as_ref(), file_content).expect("failed to parse cfg");
    for pair in sim.as_ref().split_whitespace() {
        match pair.split_once(':') {
            Some((kind, val)) => match kind {
                "t" => {
                    let tick = str::parse::<u128>(val).expect("valid num for tick");
                    k.tick_ms(tick, &None).unwrap();
                    k.can_block_update_idle_waiting(tick as u16);
                }
                "d" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Press,
                    })
                    .expect("input handles fine");
                    crate::PRESSED_KEYS.lock().insert(key_code);
                }
                "u" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Release,
                    })
                    .expect("input handles fine");
                    crate::PRESSED_KEYS.lock().remove(&key_code);
                }
                "r" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Repeat,
                    })
                    .expect("input handles fine");
                }
                _ => panic!("invalid item {pair}"),
            },
            None => panic!("invalid item {pair}"),
        }
    }
    drop(_lk);
    k.kbd_out.outputs.events.join("\n")
}

#[allow(unused)]
trait SimTransform {
    /// Changes newlines to spaces.
    fn to_spaces(self) -> Self;
    /// Removes out:↑_ items from the string. Also transforms newlines to spaces.
    fn no_releases(self) -> Self;
    /// Removes t:_ms items from the string. Also transforms newlines to spaces.
    fn no_time(self) -> Self;
    /// Replaces out:↓_ with dn:_ and out:↑_ with up:_. Also transforms newlines to spaces.
    fn to_ascii(self) -> Self;
}

impl SimTransform for String {
    fn to_spaces(self) -> Self {
        self.replace('\n', " ")
    }

    fn no_time(self) -> Self {
        self.split_ascii_whitespace()
            .filter(|s| !s.starts_with("t:"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn no_releases(self) -> Self {
        self.split_ascii_whitespace()
            .filter(|s| !s.starts_with("out:↑") && !s.starts_with("up:"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn to_ascii(self) -> Self {
        self.split_ascii_whitespace()
            .map(|s| s.replace("out:↑", "up:").replace("out:↓", "dn:"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
