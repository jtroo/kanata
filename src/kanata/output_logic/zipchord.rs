use kanata_parser::trie::Trie;

/// Tracks current input to check against possible chords.
/// This does not store by the input order;
/// instead it is by some consistent ordering for
/// hashing into the possible chord map.
struct SortedInputs {
    inputs: SortedChord,
}

/// All possible chords.
struct PossibleChords {
    chords: Trie<ChordOutput>
}
/// A chord.
///
/// If any followups exist it will be Some.
/// E.g. with:
///   - dy   -> day
///   - dy 1 -> Monday
///   - dy 2 -> Tuesday
/// the output will be "day" and the Monday+Tuesday chords will be in `followups`.
struct ChordOutput {
    output: Box<str>,
    followups: Option<PossibleChords>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// Sorted consistently by some arbitrary key order;
/// as opposed to an example of insert/input order.
struct SortedChord {
    keys: Vec<u16>,
}
impl SortedChord {
    fn insert(&mut self, key: u16) {
        match self.keys.binary_search(&key) {
            Ok(pos) => {} // Element already in vector @ `pos`. Normally this wouldn't be expected
                          // to happen but it turns out that key repeat might get in the way of
                          // this assumption.
            Err(pos) => self.keys.insert(pos, key),
        }
    }
}

enum ChordEnabledState {
    Enabled,
    Disabled,
}

struct DynamicState {
    /// Input sorted not by input order but by some consistent ordering such that it can be used to
    /// compare against a Trie.
    sorted_inputs: SortedInputs,
    /// Whether chording should be enabled or disabled.
    /// Chording will be disabled if:
    /// - further presses cannot possibly activate a chord
    /// - a release happens with no chord having been activated
    /// Once disabled, chording will be enabled when:
    /// - all keys have been released
    enabled_state: ChordEnabledState,
    /// Is Some when a chord has been activated which has possible follow-up chords.
    /// E.g. dy -> day
    ///      dy 1 -> Monday
    ///      dy 2 -> Tuesday
    /// Using the example above, when dy has been activated, the `1` and `2` activations will be
    /// contained within `prioritized_chords`. This is cleared if the input is such that an
    /// activation is no longer possible.
    prioritized_chords: Option<PossibleChords>
}

struct State {
    dynamic: DynamicState,
    /// Chords configured by the user. This is fixed at runtime other than live-reloads replacing
    /// the state.
    chords: PossibleChords,
}
