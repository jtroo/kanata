use super::*;

use kanata_parser::trie::GetOrDescendentExistsResult::*;
use kanata_parser::trie::Trie;
use rustc_hash::FxHashSet;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

/// Tracks current input to check against possible chords.
/// This does not store by the input order;
/// instead it is by some consistent ordering for
/// hashing into the possible chord map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ZchSortedInputs {
    zch_inputs: ZchSortedChord,
}
impl ZchSortedInputs {
    fn zchsi_insert(&mut self, osc: OsCode) {
        self.zch_inputs.zch_insert(osc.into());
    }
}

/// All possible chords.
#[derive(Debug, Clone, Default)]
struct ZchPossibleChords(Trie<ZchChordOutput>);
impl ZchPossibleChords {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A chord.
///
/// If any followups exist it will be Some.
/// E.g. with:
///   - dy   -> day
///   - dy 1 -> Monday
///   - dy 2 -> Tuesday
/// the output will be "day" and the Monday+Tuesday chords will be in `followups`.
#[derive(Debug, Clone)]
struct ZchChordOutput {
    zch_output: Box<str>,
    zch_followups: Option<Arc<ZchPossibleChords>>,
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
/// Sorted consistently by some arbitrary key order;
/// as opposed to an example of insert/input order.
struct ZchSortedChord {
    zch_keys: Vec<u16>,
}
impl ZchSortedChord {
    fn zch_insert(&mut self, key: u16) {
        match self.zch_keys.binary_search(&key) {
            Ok(_pos) => {} // Element already in vector @ `pos`. Normally this wouldn't be expected
            // to happen but it turns out that key repeat might get in the way of
            // this assumption.
            Err(pos) => self.zch_keys.insert(pos, key),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum ZchEnabledState {
    #[default]
    ZchEnabled,
    ZchWaitEnable,
    ZchDisabled,
}

#[derive(Debug, Default)]
struct ZchDynamicState {
    /// Input sorted not by input order but by some consistent ordering such that it can be used to
    /// compare against a Trie.
    zchd_sorted_inputs: ZchSortedInputs,
    /// Whether chording should be enabled or disabled.
    /// Chording will be disabled if:
    /// - further presses cannot possibly activate a chord
    /// - a release happens with no chord having been activated
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
    zchd_prioritized_chords: Option<Arc<ZchPossibleChords>>,
    /// Tracker for time until previous state change to know if potential stale data should be
    /// cleared.
    zchd_ticks_since_state_change: u16,
    /// Zch has a time delay between being disabled->pending-enabled->truly-enabled to mitigate
    /// against unintended activations.
    zchd_ticks_until_enabled: u16,
    /// Tracks the actually pressed keys to know when state can be reset.
    zchd_pressed_keys: FxHashSet<OsCode>,
    zchd_previous_activation_output: Option<Box<str>>,
}

impl ZchDynamicState {
    fn zchd_is_disabled(&self) -> bool {
        self.zchd_enabled_state == ZchEnabledState::ZchDisabled
    }
    fn zchd_tick(&mut self) {
        const TICKS_UNTIL_FORCE_STATE_RESET: u16 = 10000;
        self.zchd_ticks_since_state_change += 1;
        if self.zchd_enabled_state == ZchEnabledState::ZchWaitEnable {
            self.zchd_ticks_until_enabled = self.zchd_ticks_until_enabled.saturating_sub(1);
            if self.zchd_ticks_until_enabled == 0 {
                self.zchd_enabled_state = ZchEnabledState::ZchEnabled;
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
    /// Clean up the state. Not expected to be necessary to call constantly but rather only when
    /// state doesn't seem to be cleaning up on its own enough.
    fn zchd_reset(&mut self) {
        log::warn!("zchd reset state");
        self.zchd_enabled_state = ZchEnabledState::ZchEnabled;
        self.zchd_pressed_keys.clear();
        self.zchd_sorted_inputs.zch_inputs.zch_keys.clear();
        self.zchd_prioritized_chords = None;
        self.zchd_previous_activation_output = None;
    }
    /// Returns true if dynamic zch state is such that idling optimization can activate.
    fn zchd_is_idle(&self) -> bool {
        let is_idle = self.zchd_enabled_state == ZchEnabledState::ZchEnabled
            && self.zchd_pressed_keys.is_empty();
        log::trace!("zch is idle: {is_idle}");
        is_idle
    }
    fn zchd_press_key(&mut self, osc: OsCode) {
        self.zchd_pressed_keys.insert(osc);
        self.zchd_sorted_inputs.zchsi_insert(osc);
    }
    fn zchd_release_key(&mut self, osc: OsCode) {
        self.zchd_pressed_keys.remove(&osc);
        self.zchd_enabled_state = match self.zchd_pressed_keys.is_empty() {
            true => ZchEnabledState::ZchWaitEnable,
            false => ZchEnabledState::ZchDisabled,
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
    zch_cfg: ZchConfig,
}

impl ZchState {
    /// Zch handling for key presses.
    pub(crate) fn zch_press_key(
        &mut self,
        kb: &mut KbdOut,
        osc: OsCode,
    ) -> Result<(), std::io::Error> {
        if self.zch_chords.is_empty() || self.zchd.zchd_is_disabled() {
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
        let mut activation = NotInTrie;
        if let Some(pchords) = &self.zchd.zchd_prioritized_chords {
            activation = pchords
                .0
                .get_or_descendant_exists(&self.zchd.zchd_sorted_inputs.zch_inputs.zch_keys);
        }
        if !matches!(activation, HasValue(..)) {
            activation = self
                .zch_chords
                .0
                .get_or_descendant_exists(&self.zchd.zchd_sorted_inputs.zch_inputs.zch_keys);
        }
        match activation {
            HasValue(a) => {
                // TODO: delete keys associated with either the input or self.zchd.zchd_previous_activation_output
                self.zchd.zchd_previous_activation_output = Some(a.zch_output);
                self.zchd.zchd_prioritized_chords = a.zch_followups;
                todo!("type out activation.zch_output");
            }
            InTrie => {
                self.zchd
                    .zchd_sorted_inputs
                    .zch_inputs
                    .zch_insert(osc.into());
                return kb.press_key(osc);
            }
            NotInTrie => {
                self.zchd.zchd_enabled_state = ZchEnabledState::ZchDisabled;
                return kb.press_key(osc);
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
struct ZchConfig {
    zch_cfg_ticks_wait_enable: u16,
}
impl Default for ZchConfig {
    fn default() -> Self {
        Self {
            zch_cfg_ticks_wait_enable: 50,
        }
    }
}
