use super::*;

use kanata_parser::subset::GetOrIsSubsetOfKnownKey::*;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

// Maybe-todos:
// ---
// Feature-parity: smart spacing around words
//       - fixup whitespace around punctuation?
// Feature-parity: suffixes - only active while disabled, to complete a word.
// Feature-parity: prefix vs. non-prefix. Assuming smart spacing is implemented and enabled,
//                 standard activations would output space one outputs space, but not prefixes.
//                 I guess can be done in parser.

static ZCH: Lazy<Mutex<ZchState>> = Lazy::new(|| Mutex::new(Default::default()));

pub(crate) fn zch() -> MutexGuard<'static, ZchState> {
    match ZCH.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let mut inner = poisoned.into_inner();
            inner.zchd.zchd_reset();
            inner
        }
    }
}

#[derive(Debug)]
pub(crate) struct ZchConfig {
    zch_cfg_ticks_wait_enable: u16,
}
impl Default for ZchConfig {
    fn default() -> Self {
        Self {
            zch_cfg_ticks_wait_enable: 300,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum ZchEnabledState {
    #[default]
    Enabled,
    WaitEnable,
    Disabled,
}

#[derive(Debug, Default)]
struct ZchDynamicState {
    /// Input to compare against configured available chords to output.
    zchd_input_keys: ZchInputKeys,
    /// Whether chording should be enabled or disabled.
    /// Chording will be disabled if:
    /// - further presses cannot possibly activate a chord
    /// - a release happens with no chord having been activated
    ///   TODO: is the above true or even desirable?
    ///
    /// Once disabled, chording will be enabled when:
    /// - all keys have been released
    zchd_enabled_state: ZchEnabledState,
    /// Is Some when a chord has been activated which has possible follow-up chords.
    /// E.g. dy -> day
    ///      dy 1 -> Monday
    ///      dy 2 -> Tuesday
    /// Using the example above, when dy has been activated, the `1` and `2` activations will be
    /// contained within `zchd_prioritized_chords`. This is cleared if the input is such that an
    /// activation is no longer possible.
    zchd_prioritized_chords: Option<Arc<parking_lot::Mutex<ZchPossibleChords>>>,
    /// Tracks the previous output character count
    /// because it may need to be erased (see `zchd_prioritized_chords).
    zchd_previous_activation_output_count: u16,
    /// In case of output being empty for interim chord activations, this tracks the number of
    /// characters that need to be erased.
    zchd_characters_to_delete_on_next_activation: u16,
    /// Tracker for time until previous state change to know if potential stale data should be
    /// cleared. This is a contingency in case of bugs or weirdness with OS interactions, e.g.
    /// Windows lock screen weirdness.
    ///
    /// This counts upwards to a "reset state" number.
    zchd_ticks_since_state_change: u16,
    /// Zch has a time delay between being disabled->pending-enabled->truly-enabled to mitigate
    /// against unintended activations. This counts downwards from a configured number until 0, and
    /// at 0 the state transitions from pending-enabled to truly-enabled if applicable.
    zchd_ticks_until_enabled: u16,
    /// Current state of caps-word, which is a factor in handling capitalization.
    zchd_is_caps_word_active: bool,
    /// Current state of lsft which is a factor in handling capitalization.
    zchd_is_lsft_active: bool,
    /// Current state of rsft which is a factor in handling capitalization.
    zchd_is_rsft_active: bool,
}

impl ZchDynamicState {
    fn zchd_is_disabled(&self) -> bool {
        self.zchd_enabled_state == ZchEnabledState::Disabled
    }
    fn zchd_tick(&mut self, is_caps_word_active: bool) {
        const TICKS_UNTIL_FORCE_STATE_RESET: u16 = 10000;
        self.zchd_ticks_since_state_change += 1;
        self.zchd_is_caps_word_active = is_caps_word_active;
        if self.zchd_enabled_state == ZchEnabledState::WaitEnable {
            self.zchd_ticks_until_enabled = self.zchd_ticks_until_enabled.saturating_sub(1);
            if self.zchd_ticks_until_enabled == 0 {
                self.zchd_enabled_state = ZchEnabledState::Enabled;
            }
        }
        if self.zchd_ticks_since_state_change > TICKS_UNTIL_FORCE_STATE_RESET {
            self.zchd_reset();
        }
    }
    fn zchd_state_change(&mut self, cfg: &ZchConfig) {
        self.zchd_ticks_since_state_change = 0;
        self.zchd_ticks_until_enabled = cfg.zch_cfg_ticks_wait_enable;
    }

    /// Clean up the state.
    fn zchd_reset(&mut self) {
        log::debug!("zchd reset state");
        self.zchd_enabled_state = ZchEnabledState::Enabled;
        self.zchd_is_caps_word_active = false;
        self.zchd_is_lsft_active = false;
        self.zchd_is_rsft_active = false;
        self.zchd_input_keys.zchik_clear();
        self.zchd_prioritized_chords = None;
        self.zchd_previous_activation_output_count = 0;
        self.zchd_characters_to_delete_on_next_activation = 0;
    }

    /// Returns true if dynamic zch state is such that idling optimization can activate.
    fn zchd_is_idle(&self) -> bool {
        let is_idle = self.zchd_enabled_state == ZchEnabledState::Enabled
            && self.zchd_input_keys.zchik_is_empty();
        log::trace!("zch is idle: {is_idle}");
        is_idle
    }

    fn zchd_press_key(&mut self, osc: OsCode) {
        self.zchd_input_keys.zchik_insert(osc);
    }

    fn zchd_release_key(&mut self, osc: OsCode) {
        self.zchd_input_keys.zchik_remove(osc);
        self.zchd_enabled_state = match self.zchd_input_keys.zchik_is_empty() {
            true => {
                self.zchd_characters_to_delete_on_next_activation = 0;
                ZchEnabledState::WaitEnable
            }
            false => ZchEnabledState::Disabled,
        };
    }
}

#[derive(Debug, Default)]
pub(crate) struct ZchState {
    /// Dynamic state. Maybe doesn't make sense to separate this from zch_chords and to instead
    /// just flatten the structures.
    zchd: ZchDynamicState,
    /// Chords configured by the user. This is fixed at runtime other than live-reloads replacing
    /// the state.
    zch_chords: ZchPossibleChords,
    /// Options to configure behaviour.
    /// TODO: needs parser configuration.
    zch_cfg: ZchConfig,
}

impl ZchState {
    /// Configure zippychord behaviour.
    pub(crate) fn zch_configure(&mut self, chords: ZchPossibleChords) {
        self.zch_chords = chords;
        self.zchd.zchd_reset();
    }

    /// Zch handling for key presses.
    pub(crate) fn zch_press_key(
        &mut self,
        kb: &mut KbdOut,
        osc: OsCode,
    ) -> Result<(), std::io::Error> {
        match osc {
            OsCode::KEY_LEFTSHIFT => {
                self.zchd.zchd_is_lsft_active = true;
            }
            OsCode::KEY_RIGHTSHIFT => {
                self.zchd.zchd_is_rsft_active = true;
            }
            _ => {}
        }

        if self.zch_chords.is_empty() || self.zchd.zchd_is_disabled() || osc.is_modifier() {
            if osc.is_grammatical_or_structural() {
                // Motivation: if a key is pressed that can potentially be followed by a brand new
                // word, quickly re-enable zippychording so user doesn't have to wait for the
                // "not-regular-typing-anymore" timeout.
                self.zchd.zchd_enabled_state = ZchEnabledState::Enabled;
            }
            return kb.press_key(osc);
        }

        self.zchd.zchd_state_change(&self.zch_cfg);
        self.zchd.zchd_press_key(osc);

        // There might be an activation.
        // - delete typed keys
        // - output activation
        //
        // Key deletion needs to remove typed keys as well as past activations that need to be
        // cleaned up, e.g. either the previous chord in a "combo chord" or an eagerly-activated
        // chord using fewer keys, but user has still held that chord and pressed further keys,
        // activating a chord with the same+extra keys.
        let mut activation = Neither;
        if let Some(pchords) = &self.zchd.zchd_prioritized_chords {
            activation = pchords
                .lock()
                .0
                .ssm_get_or_is_subset_ksorted(self.zchd.zchd_input_keys.zchik_keys());
        }
        let mut is_prioritized_activation = false;
        if !matches!(activation, HasValue(..)) {
            activation = self
                .zch_chords
                .0
                .ssm_get_or_is_subset_ksorted(self.zchd.zchd_input_keys.zchik_keys());
        } else {
            is_prioritized_activation = true;
        }

        match activation {
            HasValue(a) => {
                if a.zch_output.is_empty() {
                    self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                    self.zchd.zchd_previous_activation_output_count =
                        self.zchd.zchd_input_keys.zchik_keys().len() as u16;
                    kb.press_key(osc)?;
                } else {
                    for _ in 0..(self.zchd.zchd_characters_to_delete_on_next_activation
                        + if is_prioritized_activation {
                            self.zchd.zchd_previous_activation_output_count
                        } else {
                            0
                        })
                    {
                        kb.press_key(OsCode::KEY_BACKSPACE)?;
                        kb.release_key(OsCode::KEY_BACKSPACE)?;
                    }
                    self.zchd.zchd_characters_to_delete_on_next_activation = 0;
                    self.zchd.zchd_previous_activation_output_count = a.zch_output.len() as u16;
                }
                self.zchd.zchd_prioritized_chords = a.zch_followups.clone();
                let mut released_lsft = false;
                for key_to_send in &a.zch_output {
                    match key_to_send {
                        ZchOutput::Lowercase(osc) => {
                            if self.zchd.zchd_input_keys.zchik_contains(*osc) {
                                kb.release_key(*osc)?;
                                kb.press_key(*osc)?;
                            } else {
                                kb.press_key(*osc)?;
                                kb.release_key(*osc)?;
                            }
                        }
                        ZchOutput::Uppercase(osc) => {
                            if !self.zchd.zchd_is_caps_word_active
                                && (released_lsft
                                    || !self.zchd.zchd_is_lsft_active
                                        && !self.zchd.zchd_is_rsft_active)
                            {
                                kb.press_key(OsCode::KEY_LEFTSHIFT)?;
                            }
                            if self.zchd.zchd_input_keys.zchik_contains(*osc) {
                                kb.release_key(*osc)?;
                                kb.press_key(*osc)?;
                            } else {
                                kb.press_key(*osc)?;
                                kb.release_key(*osc)?;
                            }
                            if !self.zchd.zchd_is_caps_word_active
                                && (released_lsft
                                    || !self.zchd.zchd_is_lsft_active
                                        && !self.zchd.zchd_is_rsft_active)
                            {
                                kb.release_key(OsCode::KEY_LEFTSHIFT)?;
                            }
                        }
                    }
                    self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                    if !released_lsft && !self.zchd.zchd_is_caps_word_active {
                        released_lsft = true;
                        if self.zchd.zchd_is_lsft_active {
                            kb.release_key(OsCode::KEY_LEFTSHIFT)?;
                        }
                        if self.zchd.zchd_is_rsft_active {
                            kb.release_key(OsCode::KEY_RIGHTSHIFT)?;
                        }
                    }
                }

                if !self.zchd.zchd_is_caps_word_active {
                    if self.zchd.zchd_is_lsft_active {
                        kb.press_key(OsCode::KEY_LEFTSHIFT)?;
                    }
                    if self.zchd.zchd_is_rsft_active {
                        kb.press_key(OsCode::KEY_RIGHTSHIFT)?;
                    }
                }

                // Note: it is incorrect to clear input keys.
                // Zippychord will eagerly output chords even if there is an overlapping chord that
                // may be activated earlier.
                // E.g.
                // ab => Abba
                // abc => Alphabet
                //
                // If (b a) are typed, "Abba" is outputted.
                // If (b a) are continued to be held and (c) is subsequently pressed,
                // "Abba" gets erased and "Alphabet" is outputted.
                //
                // WRONG:
                // self.zchd.zchd_input_keys.zchik_clear()

                Ok(())
            }

            IsSubset => {
                self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                kb.press_key(osc)
            }

            Neither => {
                self.zchd.zchd_reset();
                self.zchd.zchd_enabled_state = ZchEnabledState::Disabled;
                kb.press_key(osc)
            }
        }
    }

    // Zch handling for key releases.
    pub(crate) fn zch_release_key(
        &mut self,
        kb: &mut KbdOut,
        osc: OsCode,
    ) -> Result<(), std::io::Error> {
        match osc {
            OsCode::KEY_LEFTSHIFT => {
                self.zchd.zchd_is_lsft_active = false;
            }
            OsCode::KEY_RIGHTSHIFT => {
                self.zchd.zchd_is_rsft_active = false;
            }
            _ => {}
        }
        if self.zch_chords.is_empty() || osc.is_modifier() {
            return kb.release_key(osc);
        }
        self.zchd.zchd_state_change(&self.zch_cfg);
        self.zchd.zchd_release_key(osc);
        kb.release_key(osc)
    }

    /// Tick the zch output state.
    pub(crate) fn zch_tick(&mut self, is_caps_word_active: bool) {
        self.zchd.zchd_tick(is_caps_word_active);
    }

    /// Returns true if zch state has no further processing so the idling optimization can
    /// activate.
    pub(crate) fn zch_is_idle(&self) -> bool {
        self.zchd.zchd_is_idle()
    }
}
