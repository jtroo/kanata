use super::*;

use kanata_parser::subset::GetOrIsSubsetOfKnownKey::*;
use rustc_hash::FxHashSet;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

// TODO: suffixes - only active while disabled, to complete a word.
// TODO: prefix vs. non-prefix: one outputs space, the other not (I guess can be done in parser).
// TODO: smart spacing around words

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
    /// Tracks the previous output because it may need to be erased (see `zchd_prioritized_chords).
    zchd_previous_activation_output: Option<Box<[ZchOutput]>>,
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
    /// Tracks the actually pressed keys to know when state can be reset.
    zchd_pressed_keys: FxHashSet<OsCode>,
}

impl ZchDynamicState {
    fn zchd_is_disabled(&self) -> bool {
        self.zchd_enabled_state == ZchEnabledState::Disabled
    }
    fn zchd_tick(&mut self) {
        const TICKS_UNTIL_FORCE_STATE_RESET: u16 = 10000;
        self.zchd_ticks_since_state_change += 1;
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
        self.zchd_pressed_keys.clear();
        self.zchd_input_keys.zchik_clear();
        self.zchd_prioritized_chords = None;
        self.zchd_previous_activation_output = None;
        self.zchd_characters_to_delete_on_next_activation = 0;
    }
    /// Returns true if dynamic zch state is such that idling optimization can activate.
    fn zchd_is_idle(&self) -> bool {
        let is_idle = self.zchd_enabled_state == ZchEnabledState::Enabled
            && self.zchd_pressed_keys.is_empty();
        log::trace!("zch is idle: {is_idle}");
        is_idle
    }
    fn zchd_press_key(&mut self, osc: OsCode) {
        self.zchd_pressed_keys.insert(osc);
        self.zchd_input_keys.zchik_insert(osc);
    }
    fn zchd_release_key(&mut self, osc: OsCode) {
        self.zchd_pressed_keys.remove(&osc);
        self.zchd_input_keys.zchik_remove(osc);
        self.zchd_enabled_state = match self.zchd_pressed_keys.is_empty() {
            true => ZchEnabledState::WaitEnable,
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
        if self.zch_chords.is_empty() || self.zchd.zchd_is_disabled() || osc.is_modifier() {
            return kb.press_key(osc);
        }
        self.zchd.zchd_state_change(&self.zch_cfg);
        self.zchd.zchd_press_key(osc);
        // There might be an activation.
        // - delete typed keys
        // - output activation
        //
        // Deletion of typed keys will be based on input keys if `zchd_previous_activation_output` is
        // `None` or the previous output otherwise.
        //
        // Output activation will save into `zchd_previous_activation_output` if there is potential
        // for subsequent activations, i.e. if zch_followups is `Some`.
        let mut activation = Neither;
        if let Some(pchords) = &self.zchd.zchd_prioritized_chords {
            activation = pchords
                .lock()
                .0
                .ssm_get_or_is_subset_ksorted(self.zchd.zchd_input_keys.zchik_keys());
        }
        if !matches!(activation, HasValue(..)) {
            activation = self
                .zch_chords
                .0
                .ssm_get_or_is_subset_ksorted(self.zchd.zchd_input_keys.zchik_keys());
        }
        match activation {
            HasValue(a) => {
                if a.zch_output.is_empty() {
                    self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                    kb.press_key(osc)?;
                } else {
                    for _ in 0..self.zchd.zchd_characters_to_delete_on_next_activation {
                        kb.press_key(OsCode::KEY_BACKSPACE)?;
                        kb.release_key(OsCode::KEY_BACKSPACE)?;
                    }
                    self.zchd.zchd_characters_to_delete_on_next_activation = 0;
                }
                self.zchd.zchd_prioritized_chords = a.zch_followups.clone();
                let mut released_lsft = false;
                for key_to_send in &a.zch_output {
                    match key_to_send {
                        ZchOutput::Lowercase(osc) => {
                            if self.zchd.zchd_pressed_keys.contains(osc) {
                                kb.release_key(*osc)?;
                                self.zchd.zchd_pressed_keys.remove(osc);
                            }
                            kb.press_key(*osc)?;
                            kb.release_key(*osc)?;
                            self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                        }
                        ZchOutput::Uppercase(osc) => {
                            if self.zchd.zchd_pressed_keys.contains(osc) {
                                kb.release_key(*osc)?;
                                self.zchd.zchd_pressed_keys.remove(osc);
                            }
                            kb.press_key(OsCode::KEY_LEFTSHIFT)?;
                            kb.press_key(*osc)?;
                            kb.release_key(*osc)?;
                            kb.release_key(OsCode::KEY_LEFTSHIFT)?;
                            self.zchd.zchd_characters_to_delete_on_next_activation += 1;
                        }
                    }
                    if !released_lsft {
                        // TODO: continue to not respect shift key, but do respect caps-word in
                        // kanata. Might want to re-press shift at the end though?
                        // Also maybe don't blindly release; do so only if actually pressed?
                        released_lsft = true;
                        kb.release_key(OsCode::KEY_LEFTSHIFT)?;
                    }
                }
                self.zchd.zchd_previous_activation_output = Some(a.zch_output.clone());

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
        if self.zch_chords.is_empty() || osc.is_modifier() {
            return kb.release_key(osc);
        }
        self.zchd.zchd_state_change(&self.zch_cfg);
        self.zchd.zchd_release_key(osc);
        kb.release_key(osc)
    }
    /// Tick the zch output state.
    pub(crate) fn zch_tick(&mut self) {
        self.zchd.zchd_tick();
    }
    /// Returns true if zch state has no further processing so the idling optimization can
    /// activate.
    pub(crate) fn zch_is_idle(&self) -> bool {
        self.zchd.zchd_is_idle()
    }
}
