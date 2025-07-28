use kanata_keyberon::key_code::KeyCode;
use rustc_hash::FxHashSet as HashSet;

use kanata_parser::custom_action::CapsWordCfg;

#[derive(Debug)]
pub struct CapsWordState {
    /// Keys that will trigger an `lsft` key to be added to the active keys if present in the
    /// currently active keys.
    pub keys_to_capitalize: HashSet<KeyCode>,
    /// An extra list of keys that should **not** terminate the caps_word state, in addition to
    /// keys_to_capitalize, but which don't trigger a capitalization.
    pub keys_nonterminal: HashSet<KeyCode>,
    /// The configured timeout for caps_word.
    pub timeout: u16,
    /// The number of ticks remaining for caps_word, after which its state should be cleared. The
    /// number of ticks gets reset back to `timeout` when `tick_maybe_add_lsft` is called. The reason
    /// for having this timeout at all is in case somebody was in the middle of typing a word, had
    /// to go do something, and forgot that caps_word was active. Having this timeout means that
    /// shift won't be active for their next keypress.
    pub timeout_ticks: u16,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum CapsWordNextState {
    Active,
    End,
}

use CapsWordNextState::*;

impl CapsWordState {
    pub(crate) fn new(cfg: &CapsWordCfg) -> Self {
        Self {
            keys_to_capitalize: cfg.keys_to_capitalize.iter().copied().collect(),
            keys_nonterminal: cfg.keys_nonterminal.iter().copied().collect(),
            timeout: cfg.timeout,
            timeout_ticks: cfg.timeout,
        }
    }

    pub(crate) fn tick_maybe_add_lsft(
        &mut self,
        active_keys: &mut Vec<KeyCode>,
    ) -> CapsWordNextState {
        if self.timeout_ticks == 0 {
            log::trace!("caps-word ended");
            return End;
        }
        for kc in active_keys.iter() {
            if !self.keys_to_capitalize.contains(kc) && !self.keys_nonterminal.contains(kc) {
                return End;
            }
        }
        if active_keys
            .last()
            .map(|kc| self.keys_to_capitalize.contains(kc))
            .unwrap_or(false)
        {
            active_keys.insert(0, KeyCode::LShift);
        }
        if !active_keys.is_empty() {
            self.timeout_ticks = self.timeout;
        }
        self.timeout_ticks = self.timeout_ticks.saturating_sub(1);
        Active
    }
}
