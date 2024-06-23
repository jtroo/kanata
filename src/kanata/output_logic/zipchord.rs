use super::*;
use kanata_parser::trie::Trie;
use rustc_hash::FxHashSet;

/// Tracks current input to check against possible chords.
/// This does not store by the input order;
/// instead it is by some consistent ordering for
/// hashing into the possible chord map.
struct ZchSortedInputs {
    zch_inputs: ZchSortedChord,
}

/// All possible chords.
struct ZchPossibleChords {
    zch_chords: Trie<ZchChordOutput>,
}

/// A chord.
///
/// If any followups exist it will be Some.
/// E.g. with:
///   - dy   -> day
///   - dy 1 -> Monday
///   - dy 2 -> Tuesday
/// the output will be "day" and the Monday+Tuesday chords will be in `followups`.
struct ZchChordOutput {
    zch_output: Box<str>,
    zch_followups: Option<ZchPossibleChords>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// Sorted consistently by some arbitrary key order;
/// as opposed to an example of insert/input order.
struct ZchSortedChord {
    zch_keys: Vec<u16>,
}
impl ZchSortedChord {
    fn zch_insert(&mut self, key: u16) {
        match self.keys.binary_search(&key) {
            Ok(pos) => {} // Element already in vector @ `pos`. Normally this wouldn't be expected
            // to happen but it turns out that key repeat might get in the way of
            // this assumption.
            Err(pos) => self.keys.insert(pos, key),
        }
    }
}

enum ZchEnabledState {
    ZchEnabled,
    ZchDisabled,
}

struct ZchDynamicState {
    /// Input sorted not by input order but by some consistent ordering such that it can be used to
    /// compare against a Trie.
    zch_sorted_inputs: ZchSortedInputs,
    /// Whether chording should be enabled or disabled.
    /// Chording will be disabled if:
    /// - further presses cannot possibly activate a chord
    /// - a release happens with no chord having been activated
    /// Once disabled, chording will be enabled when:
    /// - all keys have been released
    zch_enabled_state: ZchEnabledState,
    /// Is Some when a chord has been activated which has possible follow-up chords.
    /// E.g. dy -> day
    ///      dy 1 -> Monday
    ///      dy 2 -> Tuesday
    /// Using the example above, when dy has been activated, the `1` and `2` activations will be
    /// contained within `zch_prioritized_chords`. This is cleared if the input is such that an
    /// activation is no longer possible.
    zch_prioritized_chords: Option<ZchPossibleChords>,
    /// Tracker for time until previous state change to know if potential stale data should be
    /// cleared.
    zch_ticks_since_state_change: u16,
    /// Tracks the actually pressed keys to know when state can be reset.
    zch_pressed_keys: FxHashSet<OsCode>,
}

impl ZchDynamicState {
    fn zchd_tick(&mut self) {
        const TICKS_UNTIL_FORCE_STATE_RESET: u16 = 10000;
        self.zch_ticks_since_state_change += 1;
        if self.zch_ticks_since_state_change > TICKS_UNTIL_FORCE_STATE_RESET {
            self.zchd_reset();
        }
    }
    fn zchd_reset(&mut self) {
        self.zch_enabled_state = ZchEnabledState::ZchEnabled;
        self.zch_pressed_keys.clear();
        self.zch_sorted_inputs.zch_inputs.clear();
    }
    fn zchd_is_idle(&self) -> bool {
        self.zch_enabled_state == ZchEnabledState::ZchEnabled
            && self.zch_dynamic.zch_pressed_keys.is_empty()
    }
    fn zchd_press_key(&mut self, osc: OsCode) {
        self.zch_pressed_keys.insert(osc);
        self.zch_sorted_inputs.zch_inputs.insert(osc);
    }
    fn zchd_release_key(&mut self, osc: OsCode) {
        self.zch_pressed_keys.remove(osc);
        if self.zch_pressed_keys.is_empty() {
            self.zch_enabled_state = ZchEnabledState::ZchEnabled;
        } else {
            self.zch_enabled_state = ZchEnabledState::ZchDisabled;
        }
    }
}

struct ZchState {
    /// Dynamic state. Maybe doesn't make sense to separate this from zch_chords and to instead
    /// just flatten the structures.
    zch_dynamic: ZchDynamicState,
    /// Chords configured by the user. This is fixed at runtime other than live-reloads replacing
    /// the state.
    zch_chords: ZchPossibleChords,
}

impl ZchState {
    /// Zch handling for key presses.
    pub(crate) fn zch_press_key(
        &mut self,
        kb: &mut KbdOut,
        osc: OsCode,
    ) -> Result<(), std::io::Error> {
        if self.zch_dynamic.zch_is_disabled() {
            return kb.press_key(osc);
        }
        self.zch_dynamic.zch_ticks_since_state_change = 0;
        self.zch_dynamic.zchd_press_key(osc);
        todo!()
    }
    /// Zch handling for key presses.
    pub(crate) fn zch_release_key(
        &mut self,
        kb: &mut KbdOut,
        osc: OsCode,
    ) -> Result<(), std::io::Error> {
        if self.zch_dynamic.zch_is_disabled() {
            return kb.release_key(osc);
        }
        self.zch_dynamic.zch_ticks_since_state_change = 0;
        self.zch_dynamic.zchd_release_key(osc);
        todo!()
    }
    /// Tick the zch output state.
    pub(crate) fn zch_tick(&mut self) {
        self.zch_dynamic.zchd_tick();
        todo!()
    }
    /// Returns true if zch state has no further processing so the idling optimization can
    /// activate.
    pub(crate) fn zch_is_idle(&self) -> bool {
        self.zchd_is_idle()
    }
}
