use kanata_parser::trie::Trie;

/// Tracks number of inputs used to form a chord.
/// If a chord activates, this number of backspaces will be added.
struct InputCount {}

/// Tracks current input to check against possible chords.
/// This does not store by the input order;
/// instead it is by some consistent ordering for
/// hashing into the possible chord map.
struct SortedInputs {
    inputs: SortedChord,
}

/// Used for cases of followup chord sequences from a prior activation,
/// e.g.
/// dy 1 -> Monday
/// dy 2 -> Tuesday
///
/// Inputs will be checked against this before the standard list of possible chords.
struct PrioritizePossibleChords {
    chords: Trie<Box<str>>
}

/// All possible chords.
struct PossibleChords {
    chords: Trie<Box<str>>
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
            Ok(pos) => {} // element already in vector @ `pos`
            Err(pos) => self.keys.insert(pos, key),
        }
    }
}
