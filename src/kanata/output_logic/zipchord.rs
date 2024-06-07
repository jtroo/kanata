/// Tracks number of inputs used to form a chord.
/// If a chord activates, this number of backspaces will be added.
struct InputCount {}

/// Tracks inputs to check against possible chords.
/// This does not store by the input order;
/// instead it is by some consistent ordering for
/// hashing into the possible chord map.
struct SortedInputs {}

/// Used for cases of followup chord sequences from a prior activation,
/// e.g.
/// dy 1 -> Monday
/// dy 2 -> Tuesday
///
/// Inputs will be checked against this before the standard list of possible chords.
struct PrioritizePossibleChords {}

/// All possible chords.
struct PossibleChords {}
