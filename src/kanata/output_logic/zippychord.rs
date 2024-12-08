use super::*;

use kanata_parser::subset::GetOrIsSubsetOfKnownKey::*;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

// Maybe-todos:
// ---
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
    NotChord,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ZchSmartSpaceState {
    #[default]
    Inactive,
    Sent,
}

#[derive(Debug, Default)]
struct ZchDynamicState {
    /// Input to compare against configured available chords to output.
    zchd_input_keys: ZchInputKeys,
    /// Whether chording should be enabled or disabled.
    /// Chording will be disabled if:
    /// - further presses cannot possibly activate a chord
    /// - a release happens with no chord having been activated
    ///
    /// Once disabled, chording will be enabled when:
    /// - all keys have been released
    /// - zchd_ticks_until_enabled shrinks to 0
    zchd_enabled_state: ZchEnabledState,
    /// Is Some when a chord has been activated which has possible follow-up chords.
    /// E.g. dy -> day
    ///      dy 1 -> Monday
    ///      dy 2 -> Tuesday
    /// Using the example above, when dy has been activated, the `1` and `2` activations will be
    /// contained within `zchd_prioritized_chords`. This is cleared if the input is such that an
    /// activation is no longer possible.
    zchd_prioritized_chords: Option<Arc<parking_lot::Mutex<ZchPossibleChords>>>,
    /// Tracks the prior output character count
    /// because it may need to be erased (see `zchd_prioritized_chords).
    zchd_prior_activation_output_count: i16,
    /// Tracks the number of characters typed to complete an activation, which will be erased if an
    /// activation completes succesfully.
    zchd_characters_to_delete_on_next_activation: i16,
    /// Tracks past activation for additional computation.
    zchd_prior_activation: Option<Arc<ZchChordOutput>>,
    /// Tracker for time until prior state change to know if potential stale data should be
    /// cleared. This is a contingency in case of bugs or weirdness with OS interactions, e.g.
    /// Windows lock screen weirdness.
    ///
    /// This counts upwards to a "reset state" number.
    zchd_ticks_since_state_change: u16,
    /// Zch has a time delay between being disabled->pending-enabled->truly-enabled to mitigate
    /// against unintended activations. This counts downwards from a configured number until 0, and
    /// at 0 the state transitions from pending-enabled to truly-enabled if applicable.
    zchd_ticks_until_enabled: u16,
    /// There is a deadline between the first press happening and a chord activation being
    /// possible; after which if a chord has not been activated, zippychording is disabled. This
    /// state is the counter for this deadline.
    zchd_ticks_until_disable: u16,
    /// Track number of activations within the same hold.
    zchd_same_hold_activation_count: u16,
    /// Current state of caps-word, which is a factor in handling capitalization.
    zchd_is_caps_word_active: bool,
    /// Current state of lsft which is a factor in handling capitalization.
    zchd_is_lsft_active: bool,
    /// Current state of rsft which is a factor in handling capitalization.
    zchd_is_rsft_active: bool,
    /// Current state of altgr which is a factor in smart space erasure.
    zchd_is_altgr_active: bool,
    /// Tracks whether last press was part of a chord or not.
    /// Upon releasing keys, this state determines if zippychording should remain enabled or
    /// disabled.
    zchd_last_press: ZchLastPressClassification,
    /// Tracks smart spacing state so punctuation characters
    /// can know whether a space needs to be erased or not.
    zchd_smart_space_state: ZchSmartSpaceState,
}

impl ZchDynamicState {
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
                    self.zchd_ticks_until_disable = 0;
                }
            }
            ZchEnabledState::Enabled => {
                // Only run disable-check logic if ticks is already greater than zero, because zero
                // means deadline has never been triggered by any press.
                if self.zchd_ticks_until_disable > 0 {
                    self.zchd_ticks_until_disable = self.zchd_ticks_until_disable.saturating_sub(1);
                    if self.zchd_ticks_until_disable == 0 {
                        log::debug!("zippy enable->disable");
                        self.zchd_soft_reset();
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
        self.zchd_soft_reset();
        self.zchd_is_caps_word_active = false;
        self.zchd_is_lsft_active = false;
        self.zchd_is_rsft_active = false;
        self.zchd_is_altgr_active = false;
        self.zchd_last_press = ZchLastPressClassification::IsChord;
        self.zchd_enabled_state = ZchEnabledState::Enabled;
    }

    fn zchd_soft_reset(&mut self) {
        log::debug!("zchd soft reset state");
        self.zchd_last_press = ZchLastPressClassification::NotChord;
        self.zchd_enabled_state = ZchEnabledState::Disabled;
        self.zchd_input_keys.zchik_clear();
        self.zchd_ticks_since_state_change = 0;
        self.zchd_ticks_until_disable = 0;
        self.zchd_ticks_until_enabled = 0;
        self.zchd_smart_space_state = ZchSmartSpaceState::Inactive;
        self.zchd_clear_history();
    }

    fn zchd_clear_history(&mut self) {
        log::debug!("zchd clear historical data");
        self.zchd_characters_to_delete_on_next_activation = 0;
        self.zchd_prioritized_chords = None;
        self.zchd_prior_activation = None;
        self.zchd_prior_activation_output_count = 0;
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
                self.zchd_clear_history();
            }
            (ZchLastPressClassification::NotChord, false) => {
                log::debug!("release but not all->zippy disable");
                self.zchd_soft_reset();
            }
            (ZchLastPressClassification::IsChord, true) => {
                log::debug!("all released->zippy enabled");
                if self.zchd_prioritized_chords.is_none() {
                    log::debug!("no continuation->zippy clear key erase state");
                    self.zchd_clear_history();
                }
                self.zchd_characters_to_delete_on_next_activation = 0;
                self.zchd_ticks_until_disable = 0;
                self.zchd_enabled_state = ZchEnabledState::Enabled;
                self.zchd_same_hold_activation_count = 0;
            }
            (ZchLastPressClassification::IsChord, false) => {
                log::debug!("some released->zippy enabled");
                self.zchd_ticks_until_disable = 0;
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
        if self.zch_chords.is_empty() {
            return kb.press_key(osc);
        }
        match osc {
            OsCode::KEY_LEFTSHIFT => {
                self.zchd.zchd_is_lsft_active = true;
                return kb.press_key(osc);
            }
            OsCode::KEY_RIGHTSHIFT => {
                self.zchd.zchd_is_rsft_active = true;
                return kb.press_key(osc);
            }
            OsCode::KEY_RIGHTALT => {
                self.zchd.zchd_is_altgr_active = true;
                return kb.press_key(osc);
            }
            osc if osc.is_zippy_ignored() => {
                return kb.press_key(osc);
            }
            _ => {}
        }
        if self.zchd.zchd_smart_space_state == ZchSmartSpaceState::Sent
            && self
                .zch_cfg
                .zch_cfg_smart_space_punctuation
                .contains(&match (
                    self.zchd.zchd_is_lsft_active | self.zchd.zchd_is_rsft_active,
                    self.zchd.zchd_is_altgr_active,
                ) {
                    (false, false) => ZchOutput::Lowercase(osc),
                    (true, false) => ZchOutput::Uppercase(osc),
                    (false, true) => ZchOutput::AltGr(osc),
                    (true, true) => ZchOutput::ShiftAltGr(osc),
                })
        {
            self.zchd.zchd_characters_to_delete_on_next_activation -= 1;
            kb.press_key(OsCode::KEY_BACKSPACE)?;
            kb.release_key(OsCode::KEY_BACKSPACE)?;
        }
        self.zchd.zchd_smart_space_state = ZchSmartSpaceState::Inactive;
        if self.zchd.zchd_enabled_state != ZchEnabledState::Enabled {
            return kb.press_key(osc);
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
        // cleaned up, e.g. either the antecedent in a "combo chord" or an eagerly-activated
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
                // Find the longest common prefix length between the prior activation and the new
                // activation. This value affects both:
                // - the number of backspaces that need to be done
                // - the number of characters that actually need to be typed by the activation
                let common_prefix_len_from_past_activation = if !is_prioritized_activation
                    && self.zchd.zchd_same_hold_activation_count == 0
                {
                    0
                } else {
                    self.zchd
                        .zchd_prior_activation
                        .as_ref()
                        .map(|prior_activation| {
                            let current_activation_output = &a.zch_output;
                            let mut len: i16 = 0;
                            for (past, current) in prior_activation
                                .zch_output
                                .iter()
                                .copied()
                                .zip(current_activation_output.iter().copied())
                            {
                                if past.osc() == OsCode::KEY_BACKSPACE
                                    || current.osc() == OsCode::KEY_BACKSPACE
                                    || past != current
                                {
                                    break;
                                }
                                len += 1;
                            }
                            len
                        })
                        .unwrap_or(0)
                };
                self.zchd.zchd_prior_activation = Some(a.clone());
                self.zchd.zchd_same_hold_activation_count += 1;

                self.zchd
                    .zchd_restart_deadline(self.zch_cfg.zch_cfg_ticks_chord_deadline);
                if !a.zch_output.is_empty() {
                    // Zippychording eagerly types characters that form a chord and also eagerly
                    // outputs chords that are of a maybe-to-be-activated-later chord with more
                    // participating keys. This procedure erases both classes of typed characters
                    // in order to have the correct typed output for this chord activation.
                    for _ in 0..(self.zchd.zchd_characters_to_delete_on_next_activation
                        + if is_prioritized_activation {
                            self.zchd.zchd_prior_activation_output_count
                        } else {
                            0
                        }
                        - common_prefix_len_from_past_activation)
                    {
                        kb.press_key(OsCode::KEY_BACKSPACE)?;
                        kb.release_key(OsCode::KEY_BACKSPACE)?;
                    }
                    self.zchd.zchd_characters_to_delete_on_next_activation = 0;
                    self.zchd.zchd_prior_activation_output_count =
                        ZchOutput::display_len(&a.zch_output);
                } else {
                    // Followup chords may consist of an empty output; eventually in the followup
                    // chain has an activation output that is not empty. For empty outputs, do not
                    // do any backspacing.
                    self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                    self.zchd.zchd_prior_activation_output_count +=
                        self.zchd.zchd_input_keys.zchik_keys().len() as i16;
                    kb.press_key(osc)?;
                }

                self.zchd
                    .zchd_prioritized_chords
                    .clone_from(&a.zch_followups);
                let mut released_sft = false;
                #[cfg(feature = "interception_driver")]
                let mut send_count = 0;
                if self.zchd.zchd_is_altgr_active && !a.zch_output.is_empty() {
                    kb.release_key(OsCode::KEY_RIGHTALT)?;
                }
                for key_to_send in a
                    .zch_output
                    .iter()
                    .copied()
                    .skip(common_prefix_len_from_past_activation as usize)
                {
                    #[cfg(feature = "interception_driver")]
                    {
                        // Note: every 5 keys on Windows Interception, do a sleep because
                        // sending too quickly apparently causes weird behaviour...
                        // I guess there's some buffer in the Interception code that is filling up.
                        send_count += 1;
                        if send_count % 5 == 0 {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                    }

                    match key_to_send {
                        ZchOutput::Lowercase(osc) | ZchOutput::NoEraseLowercase(osc) => {
                            type_osc(osc, kb, &self.zchd)?;
                        }
                        ZchOutput::Uppercase(osc) | ZchOutput::NoEraseUppercase(osc) => {
                            maybe_press_sft_during_activation(released_sft, kb, &self.zchd)?;
                            type_osc(osc, kb, &self.zchd)?;
                            maybe_release_sft_during_activation(released_sft, kb, &self.zchd)?;
                        }
                        ZchOutput::AltGr(osc) | ZchOutput::NoEraseAltGr(osc) => {
                            // A note regarding maybe_press|release_sft
                            // in contrast to always pressing|releasing altgr:
                            //
                            // The maybe-logic is valuable with Shift to capitalize the first
                            // typed output during activation.
                            // However, altgr - if already held -
                            // does not seem useful to keep held on the first typed output so it is
                            // always released at the beginning and pressed at the end if it was
                            // previously being held.
                            kb.press_key(OsCode::KEY_RIGHTALT)?;
                            type_osc(osc, kb, &self.zchd)?;
                            kb.release_key(OsCode::KEY_RIGHTALT)?;
                        }
                        ZchOutput::ShiftAltGr(osc) | ZchOutput::NoEraseShiftAltGr(osc) => {
                            kb.press_key(OsCode::KEY_RIGHTALT)?;
                            maybe_press_sft_during_activation(released_sft, kb, &self.zchd)?;
                            type_osc(osc, kb, &self.zchd)?;
                            maybe_release_sft_during_activation(released_sft, kb, &self.zchd)?;
                            kb.release_key(OsCode::KEY_RIGHTALT)?;
                        }
                    };

                    self.zchd.zchd_characters_to_delete_on_next_activation +=
                        key_to_send.output_char_count();

                    if !released_sft && !self.zchd.zchd_is_caps_word_active {
                        released_sft = true;
                        if self.zchd.zchd_is_lsft_active {
                            kb.release_key(OsCode::KEY_LEFTSHIFT)?;
                        }
                        if self.zchd.zchd_is_rsft_active {
                            kb.release_key(OsCode::KEY_RIGHTSHIFT)?;
                        }
                    }
                }

                if self.zch_cfg.zch_cfg_smart_space != ZchSmartSpaceCfg::Disabled
                    && a.zch_output
                        .last()
                        .map(|out| !matches!(out.osc(), OsCode::KEY_SPACE | OsCode::KEY_BACKSPACE))
                        .unwrap_or(false /* if output is empty, don't do smart spacing */)
                {
                    if self.zch_cfg.zch_cfg_smart_space == ZchSmartSpaceCfg::Full {
                        self.zchd.zchd_smart_space_state = ZchSmartSpaceState::Sent;
                    }

                    // It might look unusual to add to both.
                    // This is correct to do.
                    // zchd_prior_activation_output_count only applies to followup activations,
                    // which should only occur after a full release+repress of a new chord.
                    // The full release will set zchd_characters_to_delete_on_next_activation to 0.
                    // Overlapping chords do not use zchd_prior_activation_output_count but
                    // instead keep track of characters to delete via
                    // zchd_characters_to_delete_on_next_activation,
                    // which is incremented both by typing characters
                    // to achieve a chord in the first place,
                    // as well as by chord activations that are overlapped
                    // by the intended final chord.
                    self.zchd.zchd_prior_activation_output_count += 1;
                    self.zchd.zchd_characters_to_delete_on_next_activation += 1;

                    kb.press_key(OsCode::KEY_SPACE)?;
                    kb.release_key(OsCode::KEY_SPACE)?;
                }

                if !self.zchd.zchd_is_caps_word_active {
                    // When expanding, lsft/rsft will be released after the first press.
                    if self.zchd.zchd_is_lsft_active {
                        kb.press_key(OsCode::KEY_LEFTSHIFT)?;
                    }
                    if self.zchd.zchd_is_rsft_active {
                        kb.press_key(OsCode::KEY_RIGHTSHIFT)?;
                    }
                }
                if self.zchd.zchd_is_altgr_active && !a.zch_output.is_empty() {
                    kb.press_key(OsCode::KEY_RIGHTALT)?;
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
                self.zchd.zchd_soft_reset();
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
        if self.zch_chords.is_empty() {
            return kb.release_key(osc);
        }
        match osc {
            OsCode::KEY_LEFTSHIFT => {
                self.zchd.zchd_is_lsft_active = false;
            }
            OsCode::KEY_RIGHTSHIFT => {
                self.zchd.zchd_is_rsft_active = false;
            }
            OsCode::KEY_RIGHTALT => {
                self.zchd.zchd_is_altgr_active = false;
            }
            _ => {}
        }
        if osc.is_zippy_ignored() {
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
