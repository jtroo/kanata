//! Zipchord-like parsing. Probably not 100% compatible.
//!
//! Example lines in input file.
//! The " => " string represents a tab character.
//!
//! "dy => day"
//!   -> chord: (d y)
//!   -> output: "day"
//!
//! "dy => day"
//! "dy 1 => Monday"
//!   -> chord: (d y)
//!   -> output: "day"
//!   -> chord: (d y)
//!   -> output: "Monday"; "day" gets erased
//!
//! " abc => Alphabet"
//!   -> chord: (space a b c)
//!   -> output: "Alphabet"
//!
//! "r df => recipient"
//!   -> chord: (r)
//!   -> output: nothing yet, just type r
//!   -> chord: (d f)
//!   -> output: "recipient"
//!
//! " w  a => Washington"
//!   -> chord: (space w)
//!   -> output: nothing yet, type spacebar+w in whatever true order they were pressed
//!   -> chord: (space a)
//!   -> output: "Washington"
//!   -> note: do observe the two spaces between 'w' and 'a'
use super::*;

use crate::anyhow_expr;
use crate::bail_expr;

use std::fs;

/// All possible chords.
#[derive(Debug, Clone, Default)]
pub struct ZchPossibleChords(pub Trie<ZchChordOutput>);
impl ZchPossibleChords {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Tracks current input to check against possible chords.
/// This does not store by the input order;
/// instead it is by some consistent ordering for
/// hashing into the possible chord map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZchSortedInputs {
    zch_inputs: ZchSortedChord,
}
impl ZchSortedInputs {
    pub fn zchsi_new() -> Self {
        Self { zch_inputs: ZchSortedChord { zch_keys: Vec::new() }}
    }
    pub fn zchsi_contains(&mut self, osc: OsCode) -> bool {
        self.zch_inputs.zch_keys.contains(&osc.into())
    }
    pub fn zchsi_insert(&mut self, osc: OsCode) {
        self.zch_inputs.zch_insert(osc.into());
    }
    pub fn zchsi_len(&self) -> usize {
        self.zch_inputs.zch_keys.len()
    }
    pub fn zchsi_clear(&mut self) {
        self.zch_inputs.zch_keys.clear()
    }
    pub fn zchsi_keys(&self) -> &[u16] {
        &self.zch_inputs.zch_keys
    }
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
/// Sorted consistently by some arbitrary key order;
/// as opposed to an example of insert/input order.
pub struct ZchSortedChord {
    zch_keys: Vec<u16>,
}
impl ZchSortedChord {
    pub fn zch_insert(&mut self, key: u16) {
        match self.zch_keys.binary_search(&key) {
            Ok(_pos) => {} // Element already in vector @ `pos`. Normally this wouldn't be expected
            // to happen but it turns out that key repeat might get in the way of
            // this assumption.
            Err(pos) => self.zch_keys.insert(pos, key),
        }
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
pub struct ZchChordOutput {
    pub zch_output: Box<[ZchOutput]>,
    pub zch_followups: Option<Arc<ZchPossibleChords>>,
}

/// Zch output can be uppercase or lowercase characters.
/// The parser should ensure all `OsCode`s within `Lowercase` and `Uppercase`
/// are visible characters that can be backspaced.
#[derive(Debug, Clone, Copy)]
pub enum ZchOutput {
    Lowercase(OsCode),
    Uppercase(OsCode),
}

// TODO: implement
pub(crate) fn parse_zippy(exprs: &[SExpr], s: &ParserState) -> Result<ZchPossibleChords> {
    if exprs.len() != 2 {
        bail_expr!(
            &exprs[0],
            "There must be exactly one filename following this definition.\nFound {}",
            exprs.len() - 1
        );
    }
    let Some(file_name) = exprs[1].atom(s.vars()) else {
        bail_expr!(&exprs[1], "Filename must be a string, not a list.");
    };
    let input_data = fs::read_to_string(file_name)
        .map_err(|e| anyhow_expr!(&exprs[1], "Failed to read file:\n{e}"))?;
    input_data
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.trim().starts_with("//"))
        .try_fold(ZchPossibleChords(Trie::new()), |zch, (line_number, line)| {
            let Some((input, output)) = line.split_once('\t') else {
                bail_expr!(
                    &exprs[1],
                    "Input and output are separated by a tab, but found no tab:\n{}: {line}",
                    line_number + 1
                );
            };
            if input.is_empty() {
                bail_expr!(
                    &exprs[1],
                    "No input defined; line must not begin with a tab:\n{}: {line}",
                    line_number + 1
                );
            }
            let mut input_left_to_parse = input;
            let mut chord_chars;
            let mut input_chord = ZchSortedInputs::zchsi_new();
            let mut is_space_included;
            let mut char_buf: [u8; 4] = [0; 4];

            while !input_left_to_parse.is_empty() {
                input_chord.zchsi_clear();

                // Check for a starting space.
                (is_space_included, input_left_to_parse) = match input_left_to_parse.strip_prefix(' ') {
                    None => (false, input_left_to_parse),
                    Some(i) => (true, i),
                };
                if is_space_included {
                    input_chord.zchsi_insert(OsCode::KEY_SPACE);
                }

                // Parse chord until next space.
                (chord_chars, input_left_to_parse) = match input_left_to_parse.split_once(' ') {
                    Some(split) => split,
                    None => (input_left_to_parse, ""),
                };

                chord_chars.chars().try_fold((), |_, chord_char| -> Result<()> {
                    let key_name = chord_char.encode_utf8(&mut char_buf);
                    let osc = str_to_oscode(key_name).ok_or_else(|| {
                        anyhow_expr!(
                            &exprs[1],
                            "Found an unknown key name: {key_name}:\n{}: {line}",
                            line_number + 1
                        )
                    })?;
                    input_chord.zchsi_insert(osc);
                    Ok(())
                })?;

                // TODO: insert into possible chords
            }
            Ok(zch)
        })
}
