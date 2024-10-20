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

use crate::bail_expr;
use crate::subset::*;

use parking_lot::Mutex;

/// All possible chords.
#[derive(Debug, Clone, Default)]
pub struct ZchPossibleChords(pub SubsetMap<u16, Arc<ZchChordOutput>>);
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
pub struct ZchInputKeys {
    zch_inputs: ZchSortedChord,
}
impl ZchInputKeys {
    pub fn zchik_new() -> Self {
        Self {
            zch_inputs: ZchSortedChord {
                zch_keys: Vec::new(),
            },
        }
    }
    pub fn zchik_contains(&mut self, osc: OsCode) -> bool {
        self.zch_inputs.zch_keys.contains(&osc.into())
    }
    pub fn zchik_insert(&mut self, osc: OsCode) {
        self.zch_inputs.zch_insert(osc.into());
    }
    pub fn zchik_remove(&mut self, osc: OsCode) {
        self.zch_inputs.zch_keys.retain(|k| *k != osc.into());
    }
    pub fn zchik_len(&self) -> usize {
        self.zch_inputs.zch_keys.len()
    }
    pub fn zchik_clear(&mut self) {
        self.zch_inputs.zch_keys.clear()
    }
    pub fn zchik_keys(&self) -> &[u16] {
        &self.zch_inputs.zch_keys
    }
    pub fn zchik_is_empty(&self) -> bool {
        self.zch_inputs.zch_keys.is_empty()
    }
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
/// Sorted consistently by some arbitrary key order;
/// as opposed to, for example, simply the user press order.
pub struct ZchSortedChord {
    zch_keys: Vec<u16>,
}
impl ZchSortedChord {
    pub fn zch_insert(&mut self, key: u16) {
        match self.zch_keys.binary_search(&key) {
            // Q: what is the meaning of Ok vs. Err?
            // A: Ok means the element already in vector @ `pos`. Normally this wouldn't be
            // expected to happen but it turns out that key repeat might get in the way of this
            // assumption. Err means element does not exist and returns the correct insert position.
            Ok(_pos) => {}
            Err(pos) => self.zch_keys.insert(pos, key),
        }
    }
}

/// A chord.
///
/// If any followups exist it will be Some.
/// E.g. with:
/// - dy   -> day
/// - dy 1 -> Monday
/// - dy 2 -> Tuesday
///
/// the output will be "day" and the Monday+Tuesday chords will be in `followups`.
#[derive(Debug, Clone)]
pub struct ZchChordOutput {
    pub zch_output: Box<[ZchOutput]>,
    pub zch_followups: Option<Arc<Mutex<ZchPossibleChords>>>,
}

/// Zch output can be uppercase or lowercase characters.
/// The parser should ensure all `OsCode`s within `Lowercase` and `Uppercase`
/// are visible characters that can be backspaced.
#[derive(Debug, Clone, Copy)]
pub enum ZchOutput {
    Lowercase(OsCode),
    Uppercase(OsCode),
}

pub(crate) fn parse_zippy(
    exprs: &[SExpr],
    s: &ParserState,
    f: &mut FileContentProvider,
) -> Result<ZchPossibleChords> {
    parse_zippy_inner(exprs, s, f)
}

#[cfg(not(feature = "zippychord"))]
fn parse_zippy_inner(
    exprs: &[SExpr],
    _s: &ParserState,
    _f: &mut FileContentProvider,
) -> Result<ZchPossibleChords> {
    bail_expr!(&exprs[0], "Kanata was not compiled with the \"zippychord\" feature. This configuration is unsupported")
}

#[cfg(feature = "zippychord")]
fn parse_zippy_inner(
    exprs: &[SExpr],
    s: &ParserState,
    f: &mut FileContentProvider,
) -> Result<ZchPossibleChords> {
    use crate::anyhow_expr;
    use crate::subset::GetOrIsSubsetOfKnownKey::*;

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
    let input_data = f
        .get_file_content(file_name.as_ref())
        .map_err(|e| anyhow_expr!(&exprs[1], "Failed to read file:\n{e}"))?;
    let res = input_data
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.trim().starts_with("//"))
        .try_fold(
            Arc::new(Mutex::new(ZchPossibleChords(SubsetMap::ssm_new()))),
            |zch, (line_number, line)| {
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

                let mut char_buf: [u8; 4] = [0; 4];

                let output = {
                    output
                        .chars()
                        .try_fold(vec![], |mut zch_output, out_char| -> Result<_> {
                            let out_key = out_char.to_lowercase().next().unwrap();
                            let key_name = out_key.encode_utf8(&mut char_buf);
                            let osc = str_to_oscode(key_name).ok_or_else(|| {
                                anyhow_expr!(
                                    &exprs[1],
                                    "Unknown output key name '{}':\n{}: {line}",
                                    out_char,
                                    line_number + 1,
                                )
                            })?;
                            let out = match out_char.is_uppercase() {
                                true => ZchOutput::Uppercase(osc),
                                false => ZchOutput::Lowercase(osc),
                            };
                            zch_output.push(out);
                            Ok(zch_output)
                        })?
                        .into_boxed_slice()
                };
                let mut input_left_to_parse = input;
                let mut chord_chars;
                let mut input_chord = ZchInputKeys::zchik_new();
                let mut is_space_included;
                let mut possible_chords_map = zch.clone();
                let mut next_map: Option<Arc<Mutex<_>>>;

                while !input_left_to_parse.is_empty() {
                    input_chord.zchik_clear();

                    // Check for a starting space.
                    (is_space_included, input_left_to_parse) =
                        match input_left_to_parse.strip_prefix(' ') {
                            None => (false, input_left_to_parse),
                            Some(i) => (true, i),
                        };
                    if is_space_included {
                        input_chord.zchik_insert(OsCode::KEY_SPACE);
                    }

                    // Parse chord until next space.
                    (chord_chars, input_left_to_parse) = match input_left_to_parse.split_once(' ') {
                        Some(split) => split,
                        None => (input_left_to_parse, ""),
                    };

                    chord_chars
                        .chars()
                        .try_fold((), |_, chord_char| -> Result<()> {
                            let key_name = chord_char.encode_utf8(&mut char_buf);
                            let osc = str_to_oscode(key_name).ok_or_else(|| {
                                anyhow_expr!(
                                    &exprs[1],
                                    "Unknown input key name: '{key_name}':\n{}: {line}",
                                    line_number + 1
                                )
                            })?;
                            input_chord.zchik_insert(osc);
                            Ok(())
                        })?;

                    let output_for_input_chord = possible_chords_map
                        .lock()
                        .0
                        .ssm_get_or_is_subset_ksorted(input_chord.zchik_keys());
                    match (input_left_to_parse.is_empty(), output_for_input_chord) {
                        (true, HasValue(_)) => {
                            bail_expr!(
                            &exprs[1],
                            "Found duplicate input chord, which is disallowed {input}:\n{}: {line}",
                            line_number + 1
                        );
                        }
                        (true, _) => {
                            possible_chords_map.lock().0.ssm_insert_ksorted(
                                input_chord.zchik_keys(),
                                Arc::new(ZchChordOutput {
                                    zch_output: output,
                                    zch_followups: None,
                                }),
                            );
                            break;
                        }
                        (false, HasValue(next_nested_map)) => {
                            match &next_nested_map.zch_followups {
                                None => {
                                    let map = Arc::new(Mutex::new(ZchPossibleChords(
                                        SubsetMap::ssm_new(),
                                    )));
                                    next_map = Some(map.clone());
                                    possible_chords_map.lock().0.ssm_insert_ksorted(
                                        input_chord.zchik_keys(),
                                        ZchChordOutput {
                                            zch_output: next_nested_map.zch_output.clone(),
                                            zch_followups: Some(map),
                                        }
                                        .into(),
                                    );
                                }
                                Some(followup) => {
                                    next_map = Some(followup.clone());
                                }
                            }
                        }
                        (false, _) => {
                            let map = Arc::new(Mutex::new(ZchPossibleChords(SubsetMap::ssm_new())));
                            next_map = Some(map.clone());
                            possible_chords_map.lock().0.ssm_insert_ksorted(
                                input_chord.zchik_keys(),
                                Arc::new(ZchChordOutput {
                                    zch_output: Box::new([]),
                                    zch_followups: Some(map),
                                }),
                            );
                        }
                    };
                    if let Some(map) = next_map.take() {
                        possible_chords_map = map;
                    }
                }
                Ok(zch)
            },
        )?;
    Ok(Arc::into_inner(res).expect("no other refs").into_inner())
}
