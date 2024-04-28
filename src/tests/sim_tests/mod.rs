//! Contains tests that use simulated inputs.
//!
//! One way to write tests is to write the configuration, write the simulated input, and then let
//! the test fail by comparing the output to an empty string. Run the test then inspect the failure
//! and see if the real output looks sensible according to what is expected.

use crate::tests::*;
use kanata_state_machine::{
    oskbd::{KeyEvent, KeyValue},
    str_to_oscode, Kanata,
};

mod block_keys_tests;
mod chord_sim_tests;
mod layer_sim_tests;
mod repeat_sim_tests;
mod seq_sim_tests;

fn simulate(cfg: &str, sim: &str) -> String {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut k = Kanata::new_from_str(cfg).expect("failed to parse cfg");
    for pair in sim.split_whitespace() {
        match pair.split_once(':') {
            Some((kind, val)) => match kind {
                "t" => {
                    let tick = str::parse::<u128>(val).expect("valid num for tick");
                    k.tick_ms(tick, &None).unwrap();
                }
                "d" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Press,
                    })
                    .expect("input handles fine");
                }
                "u" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Release,
                    })
                    .expect("input handles fine");
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
    k.kbd_out.outputs.events.join("\n")
}

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
