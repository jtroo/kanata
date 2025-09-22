use super::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SequenceActivity {
    Inactive,
    Active,
}

use SequenceActivity::*;

pub struct SequenceState {
    /// Unmangled sequence of keys pressed for hidden-delay-type.
    pub raw_oscs: Vec<OsCode>,
    /// Keeps track of standard sequence state.
    /// This includes regular keys, e.g. `a b c`
    /// and chorded keys, e.g. `S-(d e f)`.
    pub sequence: Vec<u16>,
    /// Keeps track of overlapping sequence state.
    /// E.g. able to detect `O-(g h i)`
    pub overlapped_sequence: Vec<u16>,
    /// Determines the handling of keys while sequence state is in progress.
    pub sequence_input_mode: SequenceInputMode,
    /// Starts from `sequence_timeout` and ticks down
    /// approximately every millisecond.
    /// At 0 the sequence state terminates.
    pub ticks_until_timeout: u16,
    /// User-configured sequence timeout setting.
    pub sequence_timeout: u16,
    /// Whether the sequence is active or not.
    pub activity: SequenceActivity,
    /// Counter to reduce number of backspaces typed.
    noerase_count: u16,
}

impl SequenceState {
    pub fn new() -> Self {
        Self {
            raw_oscs: vec![],
            sequence: vec![],
            overlapped_sequence: vec![],
            sequence_input_mode: SequenceInputMode::HiddenSuppressed,
            ticks_until_timeout: 0,
            sequence_timeout: 0,
            activity: Inactive,
            noerase_count: 0,
        }
    }

    /// Updates the sequence state parameters, clears buffers, and sets the state to active.
    pub fn activate(&mut self, input_mode: SequenceInputMode, timeout: u16) {
        self.sequence_input_mode = input_mode;
        self.sequence_timeout = timeout;
        self.ticks_until_timeout = timeout;
        self.raw_oscs.clear();
        self.sequence.clear();
        self.overlapped_sequence.clear();
        self.activity = Active;
        self.noerase_count = 0;
    }

    pub fn is_active(&self) -> bool {
        self.activity == Active
    }

    pub fn get_active(&mut self) -> Option<&mut Self> {
        match self.activity {
            Active => Some(self),
            Inactive => None,
        }
    }

    pub fn is_inactive(&self) -> bool {
        self.activity == Inactive
    }
}

impl Default for SequenceState {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn get_mod_mask_for_cur_keys(cur_keys: &[KeyCode]) -> u16 {
    cur_keys
        .iter()
        .copied()
        .fold(0, |a, v| a | mod_mask_for_keycode(v))
}

pub(super) enum EndSequenceType {
    Standard,
    Overlap,
}

pub(super) fn do_sequence_press_logic(
    state: &mut SequenceState,
    k: &KeyCode,
    mod_mask: u16,
    kbd_out: &mut KbdOut,
    sequences: &kanata_parser::trie::Trie<(u8, u16)>,
    sequence_backtrack_modcancel: bool,
    layout: &mut BorrowedKLayout,
) -> Result<(), anyhow::Error> {
    state.ticks_until_timeout = state.sequence_timeout;
    let osc = OsCode::from(*k);
    state.raw_oscs.push(osc);
    use kanata_parser::trie::GetOrDescendentExistsResult::*;
    let pushed_into_seq = {
        // Transform to OsCode and convert modifiers other than altgr/ralt
        // (same key different names) to the left version, since that's
        // how chords get transformed when building up sequences.
        let base = u16::from(match osc {
            OsCode::KEY_RIGHTSHIFT => OsCode::KEY_LEFTSHIFT,
            OsCode::KEY_RIGHTMETA => OsCode::KEY_LEFTMETA,
            OsCode::KEY_RIGHTCTRL => OsCode::KEY_LEFTCTRL,
            osc => osc,
        });
        base | mod_mask
    };
    match state.sequence_input_mode {
        SequenceInputMode::VisibleBackspaced => {
            press_key(kbd_out, osc)?;
        }
        SequenceInputMode::HiddenSuppressed | SequenceInputMode::HiddenDelayType => {}
    }
    log::debug!("sequence got {k:?}");
    state.sequence.push(pushed_into_seq);
    let pushed_into_overlap_seq = (pushed_into_seq & MASK_KEYCODES) | KEY_OVERLAP_MARKER;
    state.overlapped_sequence.push(pushed_into_overlap_seq);
    let mut res = sequences.get_or_descendant_exists(&state.sequence);

    // Check for invalid termination of standard variant of sequence state.
    // Can potentially backtrack and overwrite modded keystates as well as overlap keystates, which
    // might exist in the sequence because of an earlier invalid termination where the standard
    // sequence got filled in with overlap sequence data.
    let mut is_invalid_termination_standard = false;
    if res == NotInTrie {
        is_invalid_termination_standard = {
            let mut no_valid_seqs = true;
            // If applicable, check again with modifier bits unset.
            for i in (0..state.sequence.len()).rev() {
                // Note: proper bounds are immediately above.
                // Can't use iter_mut due to borrowing issues.
                if state.sequence[i] == KEY_OVERLAP_MARKER {
                    state.sequence.remove(i);
                } else if sequence_backtrack_modcancel {
                    state.sequence[i] &= MASK_KEYCODES;
                } else {
                    state.sequence[i] &= !KEY_OVERLAP_MARKER;
                }
                res = sequences.get_or_descendant_exists(&state.sequence);
                if res != NotInTrie {
                    no_valid_seqs = false;
                    break;
                }
            }
            no_valid_seqs
        };
    }

    // Check for invalid termination of overlap variant of sequence state.
    // This variant does not backtrack today because I haven't figured out how to do that easily.
    // It does do some attempts to stay valid by modifying the tail of the sequence though.
    let mut res_overlapped = sequences.get_or_descendant_exists(&state.overlapped_sequence);
    let is_invalid_termination_overlapped = if res_overlapped == NotInTrie {
        // Try ending the overlapping and push overlapping seq again.
        let index_of_last = state.overlapped_sequence.len() - 1;
        state.overlapped_sequence[index_of_last] = KEY_OVERLAP_MARKER;
        state.overlapped_sequence.push(pushed_into_overlap_seq);
        res_overlapped = sequences.get_or_descendant_exists(&state.overlapped_sequence);
        let index_of_last = index_of_last + 1;
        if res_overlapped == NotInTrie {
            // Try checking the trie after setting the latest key to not have the overlapping
            // marker.
            state.overlapped_sequence[index_of_last] = pushed_into_seq;
            res_overlapped = sequences.get_or_descendant_exists(&state.overlapped_sequence);
            if res_overlapped == NotInTrie {
                if pushed_into_seq & MASK_KEYCODES == pushed_into_seq {
                    // Avoid calling get_or_descendant_exists if there is no difference, to save on
                    // doing work checking in the trie.
                    true
                } else {
                    // Try unmodded `pushed_into_seq`.
                    state.overlapped_sequence[index_of_last] = pushed_into_seq & MASK_KEYCODES;
                    res_overlapped = sequences.get_or_descendant_exists(&state.overlapped_sequence);
                    res_overlapped == NotInTrie
                }
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    match (
        is_invalid_termination_standard,
        is_invalid_termination_overlapped,
    ) {
        (false, false) => {}
        (false, true) => {
            log::debug!("overlap seq is invalid; filling with standard seq");
            // Overwrite overlapped with non-overlapped tracking
            state.overlapped_sequence.clear();
            state
                .overlapped_sequence
                .extend(state.sequence.iter().copied());
            res_overlapped = sequences.get_or_descendant_exists(&state.overlapped_sequence);
        }
        (true, false) => {
            log::debug!("standard seq is invalid; filling with overlap seq");
            state.sequence.clear();
            state
                .sequence
                .extend(state.overlapped_sequence.iter().copied());
            if state.sequence.last().copied().unwrap_or(0) != KEY_OVERLAP_MARKER
                && state.overlapped_sequence.last().copied().unwrap_or(0) >= KEY_OVERLAP_MARKER
            {
                // Always treat non-overlapping sequence as if overlap state has
                // ended; if overlapped_sequence itself has an overlap state.
                state.sequence.push(KEY_OVERLAP_MARKER);
            }
            res = sequences.get_or_descendant_exists(&state.sequence);
        }
        (true, true) => {
            // One more try for backtracking: check for validity by removing from the front.
            while res == NotInTrie && !state.sequence.is_empty() {
                state.sequence.remove(0);
                res = sequences.get_or_descendant_exists(&state.sequence);
            }
            if res == NotInTrie || state.sequence.is_empty() {
                log::debug!("invalid keys for seq");
                cancel_sequence(state, kbd_out)?;
            }
        }
    }

    // Check for successful sequence termination.
    if let HasValue((i, j)) = res_overlapped {
        // First, check for a valid simultaneous completion.
        // Simultaneous completion should take priority.
        do_successful_sequence_termination(kbd_out, state, layout, i, j, EndSequenceType::Overlap)?;
    } else if let HasValue((i, j)) = res {
        // Try terminating the overlapping and check if simultaneous termination worked.
        // Simultaneous completion should take priority.
        state.overlapped_sequence.push(KEY_OVERLAP_MARKER);
        if let HasValue((oi, oj)) = sequences.get_or_descendant_exists(&state.overlapped_sequence) {
            do_successful_sequence_termination(
                kbd_out,
                state,
                layout,
                oi,
                oj,
                EndSequenceType::Overlap,
            )?;
        } else {
            do_successful_sequence_termination(
                kbd_out,
                state,
                layout,
                i,
                j,
                EndSequenceType::Standard,
            )?;
        }
    }
    Ok(())
}

use kanata_keyberon::key_code::KeyCode::*;

pub(super) fn do_successful_sequence_termination(
    kbd_out: &mut KbdOut,
    state: &mut SequenceState,
    layout: &mut Layout<'_, 767, 2, &&[&CustomAction]>,
    i: u8,
    j: u16,
    seq_type: EndSequenceType,
) -> Result<(), anyhow::Error> {
    log::debug!("sequence complete; tapping fake key");
    state.activity = Inactive;
    let sequence = match seq_type {
        EndSequenceType::Standard => &state.sequence,
        EndSequenceType::Overlap => &state.overlapped_sequence,
    };
    match state.sequence_input_mode {
        SequenceInputMode::HiddenSuppressed | SequenceInputMode::HiddenDelayType => {}
        SequenceInputMode::VisibleBackspaced => {
            // Release mod keys and backspace because they can cause backspaces to mess up.
            layout.states.retain(|s| match s {
                State::NormalKey { keycode, .. } => {
                    if matches!(keycode, LCtrl | RCtrl | LAlt | RAlt | LGui | RGui) {
                        // Ignore the error, ugly to return it from retain, and
                        // this is very unlikely to happen anyway.
                        let _ = release_key(kbd_out, keycode.into());
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            });
            for k in sequence.iter().copied() {
                // Check for pressed modifiers and don't input backspaces for
                // those since they don't output characters that can be
                // backspaced.
                if k == KEY_OVERLAP_MARKER {
                    continue;
                };
                let osc = OsCode::from(k & MASK_KEYCODES);
                match osc {
                    // Known bug: most non-characters-outputting keys are not
                    // listed. I'm too lazy to list them all. Just use
                    // character-outputting keys (and modifiers) in sequences
                    // please! Or switch to a different input mode? It doesn't
                    // really make sense to use non-typing characters other
                    // than modifiers does it? Since those would probably be
                    // further away from the home row, so why use them? If one
                    // desired to fix this, a shorter list of keys would
                    // probably be the list of keys that **do** output
                    // characters than those that don't.
                    osc if osc.is_modifier() => continue,
                    osc if matches!(u16::from(osc), KEY_IGNORE_MIN..=KEY_IGNORE_MAX) => continue,
                    _ => {
                        if state.noerase_count > 0 {
                            state.noerase_count -= 1;
                        } else {
                            kbd_out.press_key(OsCode::KEY_BACKSPACE)?;
                            kbd_out.release_key(OsCode::KEY_BACKSPACE)?;
                        }
                    }
                }
            }
        }
    }
    for k in sequence.iter().copied() {
        if k == KEY_OVERLAP_MARKER {
            continue;
        };
        let kc = KeyCode::from(OsCode::from(k & MASK_KEYCODES));
        layout.states.retain(|s| match s {
            State::NormalKey { keycode, .. } => kc != *keycode,
            _ => true,
        });
    }
    layout.event_to_front(Event::Release(i, j));
    layout.event_to_front(Event::Press(i, j));
    Ok(())
}

pub(super) fn cancel_sequence(state: &mut SequenceState, kbd_out: &mut KbdOut) -> Result<()> {
    state.activity = Inactive;
    log::debug!("sequence cancelled");
    match state.sequence_input_mode {
        SequenceInputMode::HiddenDelayType => {
            for osc in state.raw_oscs.iter().copied() {
                // BUG: chorded_hidden_delay_type
                press_key(kbd_out, osc)?;
                release_key(kbd_out, osc)?;
            }
        }
        SequenceInputMode::HiddenSuppressed | SequenceInputMode::VisibleBackspaced => {}
    }
    Ok(())
}

pub(super) fn add_noerase(state: &mut SequenceState, noerase_count: u16) {
    state.noerase_count += noerase_count;
}
