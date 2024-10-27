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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum ZchEnabledState {
    #[default]
    Enabled,
    WaitEnable,
    Disabled,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ZchLastPressClassification {
    #[default]
    IsChord,
    IsQuickEnable,
    NotChord,
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
    /// Zch has a time delay between being disabled->pending-enabled->truly-enabled to mitigate
    /// against unintended activations. This counts downwards from a configured number until 0, and
    /// at 0 the state transitions from pending-enabled to truly-enabled if applicable.
    zchd_ticks_until_disable: u16,
    /// Current state of caps-word, which is a factor in handling capitalization.
    zchd_is_caps_word_active: bool,
    /// Current state of lsft which is a factor in handling capitalization.
    zchd_is_lsft_active: bool,
    /// Current state of rsft which is a factor in handling capitalization.
    zchd_is_rsft_active: bool,
    /// Tracks whether last press was part of a chord or not.
    zchd_last_press: ZchLastPressClassification,
}

impl ZchDynamicState {
    fn zchd_is_disabled(&self) -> bool {
        matches!(
            self.zchd_enabled_state,
            ZchEnabledState::Disabled | ZchEnabledState::WaitEnable
        )
    }

    fn zchd_tick(&mut self, is_caps_word_active: bool) {
        const TICKS_UNTIL_FORCE_STATE_RESET: u16 = 10000;
        self.zchd_ticks_since_state_change += 1;
        self.zchd_is_caps_word_active = is_caps_word_active;
        match self.zchd_enabled_state {
            ZchEnabledState::WaitEnable => {
                self.zchd_ticks_until_enabled = self.zchd_ticks_until_enabled.saturating_sub(1);
                if self.zchd_ticks_until_enabled == 0 {
                    log::debug!("zippy wait enable->enable");
                    self.zchd_enabled_state = ZchEnabledState::Enabled;
                }
            }
            ZchEnabledState::Enabled => {
                // Only run disable-check logic if ticks is already greater than zero, because zero
                // means deadline has never been triggered by an press yet.
                if self.zchd_ticks_until_disable > 0 {
                    self.zchd_ticks_until_disable = self.zchd_ticks_until_disable.saturating_sub(1);
                    if self.zchd_ticks_until_disable == 0 {
                        log::debug!("zippy enable->disable");
                        self.zchd_enabled_state = ZchEnabledState::Disabled;
                    }
                }
            }
            ZchEnabledState::Disabled => {}
        }
        if self.zchd_ticks_since_state_change > TICKS_UNTIL_FORCE_STATE_RESET {
            self.zchd_reset();
        }
    }

    fn zchd_state_change(&mut self, cfg: &ZchConfig) {
        self.zchd_ticks_since_state_change = 0;
        self.zchd_ticks_until_enabled = cfg.zch_cfg_ticks_wait_enable;
    }

    fn zchd_activate_chord_deadline(&mut self, deadline_ticks: u16) {
        if self.zchd_ticks_until_disable == 0 {
            self.zchd_ticks_until_disable = deadline_ticks;
        }
    }

    fn zchd_restart_deadline(&mut self, deadline_ticks: u16) {
        self.zchd_ticks_until_disable = deadline_ticks;
    }

    /// Clean up the state, potentially causing inaccuracies with regards to what the user is
    /// currently still pressing.
    fn zchd_reset(&mut self) {
        log::debug!("zchd reset state");
        self.zchd_is_caps_word_active = false;
        self.zchd_is_lsft_active = false;
        self.zchd_is_rsft_active = false;
        self.zchd_soft_reset();
    }

    fn zchd_soft_reset(&mut self) {
        log::debug!("zchd soft reset state");
        self.zchd_enabled_state = ZchEnabledState::Enabled;
        self.zchd_last_press = ZchLastPressClassification::IsChord;
        self.zchd_input_keys.zchik_clear();
        self.zchd_prioritized_chords = None;
        self.zchd_previous_activation_output_count = 0;
        self.zchd_ticks_since_state_change = 0;
        self.zchd_ticks_until_disable = 0;
        self.zchd_ticks_until_enabled = 0;
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
        match (self.zchd_last_press, self.zchd_input_keys.zchik_is_empty()) {
            (ZchLastPressClassification::NotChord, true) => {
                log::debug!("all released->zippy wait enable");
                self.zchd_enabled_state = ZchEnabledState::WaitEnable;
                self.zchd_characters_to_delete_on_next_activation = 0;
            }
            (ZchLastPressClassification::NotChord, false) => {
                log::debug!("release but not all->zippy disable");
                self.zchd_enabled_state = ZchEnabledState::Disabled;
            }
            (ZchLastPressClassification::IsChord, true) => {
                log::debug!("all released->zippy enabled");
                if self.zchd_prioritized_chords.is_none() {
                    log::debug!("no continuation->zippy clear key erase state");
                    self.zchd_previous_activation_output_count = 0;
                }
                self.zchd_characters_to_delete_on_next_activation = 0;
                self.zchd_ticks_until_disable = 0;
            }
            (ZchLastPressClassification::IsChord, false) => {
                log::debug!("some released->zippy enabled");
                self.zchd_ticks_until_disable = 0;
            }
            (ZchLastPressClassification::IsQuickEnable, _) => {
                log::debug!("quick enable release->clear characters");
                self.zchd_previous_activation_output_count = 0;
                self.zchd_characters_to_delete_on_next_activation = 0;
            }
        }
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
    zch_cfg: ZchConfig,
}

impl ZchState {
    /// Configure zippychord behaviour.
    pub(crate) fn zch_configure(&mut self, cfg: (ZchPossibleChords, ZchConfig)) {
        self.zch_chords = cfg.0;
        self.zch_cfg = cfg.1;
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

        if self.zch_chords.is_empty() || osc.is_modifier() {
            return kb.press_key(osc);
        }
        if osc_triggers_quick_enable(osc) {
            if self.zchd.zchd_is_disabled() {
                log::debug!("zippy quick enable");
                // Motivation: if a key is pressed that can potentially be followed by a brand new
                // word, quickly re-enable zippychording so user doesn't have to wait for the
                // "not-regular-typing-anymore" timeout.
                self.zchd.zchd_soft_reset();
                return kb.press_key(osc);
            } else {
                self.zchd.zchd_last_press = ZchLastPressClassification::IsQuickEnable;
            }
        }

        // Zippychording is enabled. Ensure the deadline to disable it if no chord activates is
        // active.
        self.zchd
            .zchd_activate_chord_deadline(self.zch_cfg.zch_cfg_ticks_chord_deadline);
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
                self.zchd
                    .zchd_restart_deadline(self.zch_cfg.zch_cfg_ticks_chord_deadline);
                if a.zch_output.is_empty() {
                    self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                    self.zchd.zchd_previous_activation_output_count +=
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
                            type_osc(*osc, kb, &self.zchd)?;
                        }
                        ZchOutput::Uppercase(osc) => {
                            maybe_press_sft_during_activation(released_lsft, kb, &self.zchd)?;
                            type_osc(*osc, kb, &self.zchd)?;
                            maybe_release_sft_during_activation(released_lsft, kb, &self.zchd)?;
                        }
                        ZchOutput::AltGr(osc) => {
                            // Note, unlike shift which probably has a good reason to be maybe
                            // already held during chording, I don't currently see ralt as having
                            // any reason to already be held during chording; just use normal
                            // characters.
                            kb.press_key(OsCode::KEY_RIGHTALT)?;
                            type_osc(*osc, kb, &self.zchd)?;
                            kb.release_key(OsCode::KEY_RIGHTALT)?;
                        }
                        ZchOutput::ShiftAltGr(osc) => {
                            kb.press_key(OsCode::KEY_RIGHTALT)?;
                            maybe_press_sft_during_activation(released_lsft, kb, &self.zchd)?;
                            type_osc(*osc, kb, &self.zchd)?;
                            maybe_release_sft_during_activation(released_lsft, kb, &self.zchd)?;
                            kb.release_key(OsCode::KEY_RIGHTALT)?;
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
                // may be activated later by an additional keypress before any releases happen.
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

                self.zchd.zchd_last_press = ZchLastPressClassification::IsChord;
                Ok(())
            }

            IsSubset => {
                self.zchd.zchd_last_press = ZchLastPressClassification::NotChord;
                self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                kb.press_key(osc)
            }

            Neither => {
                self.zchd.zchd_last_press = ZchLastPressClassification::NotChord;
                self.zchd.zchd_soft_reset();
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

/// Currently only returns true if the key is space.
fn osc_triggers_quick_enable(osc: OsCode) -> bool {
    matches!(osc, OsCode::KEY_SPACE)
    // Old implementation.
    // ~~Returns true if punctuation or whitespace. Also backspace, delete, arrow keys.~~
    // OsCode::KEY_BACKSPACE
    //     | OsCode::KEY_DELETE
    //     | OsCode::KEY_ENTER
    //     | OsCode::KEY_SPACE
    //     | OsCode::KEY_TAB
    //     | OsCode::KEY_COMMA
    //     | OsCode::KEY_DOT
    //     | OsCode::KEY_SEMICOLON
    //     | OsCode::KEY_APOSTROPHE
    //     | OsCode::KEY_SLASH
    //     | OsCode::KEY_BACKSLASH
    //     | OsCode::KEY_GRAVE
    //     | OsCode::KEY_MINUS
    //     | OsCode::KEY_LEFTBRACE
    //     | OsCode::KEY_RIGHTBRACE
    //     | OsCode::KEY_UP
    //     | OsCode::KEY_DOWN
    //     | OsCode::KEY_LEFT
    //     | OsCode::KEY_RIGHT
    //     | OsCode::KEY_HOME
    //     | OsCode::KEY_END
    //     | OsCode::KEY_PAGEUP
    //     | OsCode::KEY_PAGEDOWN
}

fn type_osc(osc: OsCode, kb: &mut KbdOut, zchd: &ZchDynamicState) -> Result<(), std::io::Error> {
    if zchd.zchd_input_keys.zchik_contains(osc) {
        kb.release_key(osc)?;
        kb.press_key(osc)?;
    } else {
        kb.press_key(osc)?;
        kb.release_key(osc)?;
    }
    Ok(())
}

fn maybe_press_sft_during_activation(
    sft_already_released: bool,
    kb: &mut KbdOut,
    zchd: &ZchDynamicState,
) -> Result<(), std::io::Error> {
    if !zchd.zchd_is_caps_word_active
        && (sft_already_released || !zchd.zchd_is_lsft_active && !zchd.zchd_is_rsft_active)
    {
        kb.press_key(OsCode::KEY_LEFTSHIFT)?;
    }
    Ok(())
}

fn maybe_release_sft_during_activation(
    sft_already_released: bool,
    kb: &mut KbdOut,
    zchd: &ZchDynamicState,
) -> Result<(), std::io::Error> {
    if !zchd.zchd_is_caps_word_active
        && (sft_already_released || !zchd.zchd_is_lsft_active && !zchd.zchd_is_rsft_active)
    {
        kb.release_key(OsCode::KEY_LEFTSHIFT)?;
    }
    Ok(())
}
