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

#[cfg(not(feature = "zippychord"))]
#[derive(Debug, Clone, Default)]
pub struct ZchPossibleChords();
#[cfg(not(feature = "zippychord"))]
#[derive(Debug, Clone, Default)]
pub struct ZchConfig();
#[cfg(not(feature = "zippychord"))]
fn parse_zippy_inner(
    exprs: &[SExpr],
    _s: &ParserState,
    _f: &mut FileContentProvider,
) -> Result<(ZchPossibleChords, ZchConfig)> {
    bail_expr!(&exprs[0], "Kanata was not compiled with the \"zippychord\" feature. This configuration is unsupported")
}

pub(crate) fn parse_zippy(
    exprs: &[SExpr],
    s: &ParserState,
    f: &mut FileContentProvider,
) -> Result<(ZchPossibleChords, ZchConfig)> {
    parse_zippy_inner(exprs, s, f)
}

#[cfg(feature = "zippychord")]
pub use inner::*;
#[cfg(feature = "zippychord")]
mod inner {
    use super::*;

    use crate::anyhow_expr;
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
        pub fn zchik_contains(&self, osc: OsCode) -> bool {
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

    /// Zch output can be uppercase, lowercase, altgr, and shift-altgr characters.
    /// The parser should ensure all `OsCode`s in variants containing them
    /// are visible characters that are backspacable.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum ZchOutput {
        Lowercase(OsCode),
        Uppercase(OsCode),
        AltGr(OsCode),
        ShiftAltGr(OsCode),
        NoEraseLowercase(OsCode),
        NoEraseUppercase(OsCode),
        NoEraseAltGr(OsCode),
        NoEraseShiftAltGr(OsCode),
    }

    impl ZchOutput {
        pub fn osc(self) -> OsCode {
            use ZchOutput::*;
            match self {
                Lowercase(osc)
                | Uppercase(osc)
                | AltGr(osc)
                | ShiftAltGr(osc)
                | NoEraseLowercase(osc)
                | NoEraseUppercase(osc)
                | NoEraseAltGr(osc)
                | NoEraseShiftAltGr(osc) => osc,
            }
        }
        pub fn osc_and_is_noerase(self) -> (OsCode, bool) {
            use ZchOutput::*;
            match self {
                Lowercase(osc) | Uppercase(osc) | AltGr(osc) | ShiftAltGr(osc) => (osc, false),
                NoEraseLowercase(osc)
                | NoEraseUppercase(osc)
                | NoEraseAltGr(osc)
                | NoEraseShiftAltGr(osc) => (osc, true),
            }
        }
        pub fn display_len(outs: impl AsRef<[Self]>) -> i16 {
            outs.as_ref().iter().copied().fold(0i16, |mut len, out| {
                len += out.output_char_count();
                len
            })
        }
        pub fn output_char_count(self) -> i16 {
            match self.osc_and_is_noerase() {
                (OsCode::KEY_BACKSPACE, _) => -1,
                (_, false) => 1,
                (_, true) => 0,
            }
        }
    }

    /// User configuration for smart space.
    ///
    /// - `Full`         = add spaces after words, remove these spaces after typing punctuation.
    /// - `AddSpaceOnly` = add spaces after words
    /// - `Disabled`     = do nothing
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ZchSmartSpaceCfg {
        Full,
        AddSpaceOnly,
        Disabled,
    }

    #[derive(Debug)]
    pub struct ZchConfig {
        /// When, during typing, chord fails to activate, zippychord functionality becomes temporarily
        /// disabled. This is to avoid accidental chord activations when typing normally, as opposed to
        /// intentionally trying to activate a chord. The duration of temporary disabling is determined
        /// by this configuration item. Re-enabling also happens when word-splitting characters are
        /// typed, for example typing  a space or a comma, but a pause of all typing activity lasting a
        /// number of milliseconds equal to this configuration will also re-enable chording even if
        /// typing within a single word.
        pub zch_cfg_ticks_wait_enable: u16,

        /// Assuming zippychording is enabled, when the first press happens this deadline will begin
        /// and if no chords are completed within the deadline, zippychording will be disabled
        /// temporarily (see `zch_cfg_ticks_wait_enable`). You may want a long or short deadline
        /// depending on your use case. If you are primarily typing normally, with chords being used
        /// occasionally being used, you may want a short deadline so that regular typing will be
        /// unlikely to activate any chord. However, if you primarily type with chords, you may want a
        /// longer deadline to give you more time to complete the intended chord (e.g. in case of
        /// overlaps). With a long deadline you should be very intentional about pressing and releasing
        /// an individual key to begin a sequence of regular typing to trigger the disabling of
        /// zippychord. If, after the first press, a chord activates, this deadline will reset to
        /// enable further chord activations.
        pub zch_cfg_ticks_chord_deadline: u16,

        /// User configuration for smart space. See `pub enum ZchSmartSpaceCfg`.
        pub zch_cfg_smart_space: ZchSmartSpaceCfg,

        /// Define keys for punctuation, which is relevant to smart space auto-erasure of added spaces.
        pub zch_cfg_smart_space_punctuation: HashSet<ZchOutput>,
    }

    impl Default for ZchConfig {
        fn default() -> Self {
            Self {
                zch_cfg_ticks_wait_enable: 500,
                zch_cfg_ticks_chord_deadline: 500,
                zch_cfg_smart_space: ZchSmartSpaceCfg::Disabled,
                zch_cfg_smart_space_punctuation: {
                    let mut puncs = HashSet::default();
                    puncs.insert(ZchOutput::Lowercase(OsCode::KEY_DOT));
                    puncs.insert(ZchOutput::Lowercase(OsCode::KEY_COMMA));
                    puncs.insert(ZchOutput::Lowercase(OsCode::KEY_SEMICOLON));
                    puncs.shrink_to_fit();
                    puncs
                },
            }
        }
    }

    const NO_ERASE: &str = "no-erase";
    const SINGLE_OUTPUT_MULTI_KEY: &str = "single-output";

    enum ZchIoMappingType {
        NoErase,
        SingleOutput,
    }
    impl ZchIoMappingType {
        fn try_parse(expr: &SExpr, vars: Option<&HashMap<String, SExpr>>) -> Result<Self> {
            use ZchIoMappingType::*;
            expr.atom(vars)
                .and_then(|name| match name {
                    NO_ERASE => Some(NoErase),
                    SINGLE_OUTPUT_MULTI_KEY => Some(SingleOutput),
                    _ => None,
                })
                .ok_or_else(|| {
                    anyhow_expr!(
                        &expr,
                        "Unknown output type. Must be one of:\nno-erase | single-output"
                    )
                })
        }
    }

    #[cfg(feature = "zippychord")]
    pub(super) fn parse_zippy_inner(
        exprs: &[SExpr],
        s: &ParserState,
        f: &mut FileContentProvider,
    ) -> Result<(ZchPossibleChords, ZchConfig)> {
        use crate::subset::GetOrIsSubsetOfKnownKey::*;

        if exprs[0].atom(None).expect("should be atom") == "defzippy-experimental" {
            log::warn!(
                "You should replace defzippy-experimental with defzippy.\n\
             Using -experimental will be invalid in the future."
            );
        }

        if exprs.len() < 2 {
            bail_expr!(
                &exprs[0],
                "There must be a filename following the zippy definition.\nFound {}",
                exprs.len() - 1
            );
        }

        let Some(file_name) = exprs[1].atom(s.vars()) else {
            bail_expr!(&exprs[1], "Filename must be a string, not a list.");
        };

        let mut config = ZchConfig::default();

        const KEY_NAME_MAPPINGS: &str = "output-character-mappings";
        const IDLE_REACTIVATE_TIME: &str = "idle-reactivate-time";
        const CHORD_DEADLINE: &str = "on-first-press-chord-deadline";
        const SMART_SPACE: &str = "smart-space";
        const SMART_SPACE_PUNCTUATION: &str = "smart-space-punctuation";

        let mut idle_reactivate_time_seen = false;
        let mut key_name_mappings_seen = false;
        let mut chord_deadline_seen = false;
        let mut smart_space_seen = false;
        let mut smart_space_punctuation_seen = false;
        let mut smart_space_punctuation_val_expr = None;

        let mut user_cfg_char_to_output: HashMap<char, Vec<ZchOutput>> = HashMap::default();
        let mut pairs = exprs[2..].chunks_exact(2);
        for pair in pairs.by_ref() {
            let config_name = &pair[0];
            let config_value = &pair[1];

            match config_name.atom(s.vars()).ok_or_else(|| {
                anyhow_expr!(
                    config_name,
                    "A configuration name must be a string, not a list"
                )
            })? {
                IDLE_REACTIVATE_TIME => {
                    if idle_reactivate_time_seen {
                        bail_expr!(
                            config_name,
                            "This is the 2nd instance; it can only be defined once"
                        );
                    }
                    idle_reactivate_time_seen = true;
                    config.zch_cfg_ticks_wait_enable =
                        parse_u16(config_value, s, IDLE_REACTIVATE_TIME)?;
                }

                CHORD_DEADLINE => {
                    if chord_deadline_seen {
                        bail_expr!(
                            config_name,
                            "This is the 2nd instance; it can only be defined once"
                        );
                    }
                    chord_deadline_seen = true;
                    config.zch_cfg_ticks_chord_deadline =
                        parse_u16(config_value, s, CHORD_DEADLINE)?;
                }

                SMART_SPACE => {
                    if smart_space_seen {
                        bail_expr!(
                            config_name,
                            "This is the 2nd instance; it can only be defined once"
                        );
                    }
                    smart_space_seen = true;
                    config.zch_cfg_smart_space = config_value
                        .atom(s.vars())
                        .and_then(|val| match val {
                            "none" => Some(ZchSmartSpaceCfg::Disabled),
                            "full" => Some(ZchSmartSpaceCfg::Full),
                            "add-space-only" => Some(ZchSmartSpaceCfg::AddSpaceOnly),
                            _ => None,
                        })
                        .ok_or_else(|| {
                            anyhow_expr!(&config_value, "Must be: none | full | add-space-only")
                        })?;
                }

                SMART_SPACE_PUNCTUATION => {
                    if smart_space_punctuation_seen {
                        bail_expr!(
                            config_name,
                            "This is the 2nd instance; it can only be defined once"
                        );
                    }
                    smart_space_punctuation_seen = true;
                    // Need to save and parse this later since it makes use of KEY_NAME_MAPPINGS.
                    smart_space_punctuation_val_expr = Some(config_value);
                }

                KEY_NAME_MAPPINGS => {
                    if key_name_mappings_seen {
                        bail_expr!(
                            config_name,
                            "This is the 2nd instance; it can only be defined once"
                        );
                    }
                    key_name_mappings_seen = true;
                    let mut mappings = config_value
                        .list(s.vars())
                        .ok_or_else(|| {
                            anyhow_expr!(
                                config_value,
                                "{KEY_NAME_MAPPINGS} must be followed by a list"
                            )
                        })?
                        .chunks_exact(2);

                    for mapping_pair in mappings.by_ref() {
                        let input = mapping_pair[0]
                            .atom(None)
                            .ok_or_else(|| {
                                anyhow_expr!(
                                    &mapping_pair[0],
                                    "key mapping input does not use lists"
                                )
                            })?
                            .trim_atom_quotes();
                        if input.chars().count() != 1 {
                            bail_expr!(&mapping_pair[0], "Inputs should be exactly one character");
                        }
                        let input_char = input.chars().next().expect("count is 1");

                        let output = match mapping_pair[1].atom(s.vars()) {
                            Some(o) => vec![parse_single_zippy_output_mapping(
                                o,
                                &mapping_pair[1],
                                false,
                            )?],
                            None => {
                                // note for unwrap below: must be list if not atom
                                let output_list = mapping_pair[1].list(s.vars()).unwrap();
                                if output_list.is_empty() {
                                    bail_expr!(
                                        &mapping_pair[1],
                                        "Empty list is invalid for zippy output mapping."
                                    );
                                }
                                let output_type =
                                    ZchIoMappingType::try_parse(&output_list[0], s.vars())?;
                                match output_type {
                                    ZchIoMappingType::NoErase => {
                                        const ERR: &str = "expects a single key or output chord.";
                                        if output_list.len() != 2 {
                                            anyhow_expr!(&output_list[1], "{NO_ERASE} {ERR}");
                                        }
                                        let output =
                                            output_list[1].atom(s.vars()).ok_or_else(|| {
                                                anyhow_expr!(&output_list[1], "{NO_ERASE} {ERR}")
                                            })?;
                                        vec![parse_single_zippy_output_mapping(
                                            output,
                                            &output_list[1],
                                            true,
                                        )?]
                                    }
                                    ZchIoMappingType::SingleOutput => {
                                        if output_list.len() < 2 {
                                            anyhow_expr!(&output_list[1], "{SINGLE_OUTPUT_MULTI_KEY} expects one or more keys or output chords.");
                                        }
                                        let all_params_except_last =
                                            &output_list[1..output_list.len() - 1];
                                        let mut outs = vec![];
                                        for expr in all_params_except_last {
                                            let output = expr
                                            .atom(s.vars())
                                            .ok_or_else(|| {
                                                anyhow_expr!(&output_list[1], "{SINGLE_OUTPUT_MULTI_KEY} does not allow list parameters.")
                                            })?;
                                            let out = parse_single_zippy_output_mapping(
                                                output,
                                                &output_list[1],
                                                true,
                                            )?;
                                            outs.push(out);
                                        }
                                        let last_expr = &output_list.last().unwrap(); // non-empty, checked length already
                                        let last_out = last_expr
                                        .atom(s.vars())
                                        .ok_or_else(|| {
                                            anyhow_expr!(last_expr, "{SINGLE_OUTPUT_MULTI_KEY} does not allow list parameters.")
                                        })?;
                                        outs.push(parse_single_zippy_output_mapping(
                                            last_out, last_expr, false,
                                        )?);
                                        outs
                                    }
                                }
                            }
                        };

                        if user_cfg_char_to_output.insert(input_char, output).is_some() {
                            bail_expr!(&mapping_pair[0], "Duplicate character, not allowed");
                        }
                    }

                    let rem = mappings.remainder();
                    if !rem.is_empty() {
                        bail_expr!(&rem[0], "zippy input is missing its output mapping");
                    }
                }
                _ => bail_expr!(config_name, "Unknown zippy configuration name"),
            }
        }

        let rem = pairs.remainder();
        if !rem.is_empty() {
            bail_expr!(&rem[0], "zippy config name is missing its value");
        }

        if let Some(val) = smart_space_punctuation_val_expr {
            config.zch_cfg_smart_space_punctuation = val
                .list(s.vars())
                .ok_or_else(|| {
                    anyhow_expr!(val, "{SMART_SPACE_PUNCTUATION} must be followed by a list")
                })?
                .iter()
                .try_fold(vec![], |mut puncs, punc_expr| -> Result<Vec<ZchOutput>> {
                    let punc = punc_expr
                        .atom(s.vars())
                        .ok_or_else(|| anyhow_expr!(&punc_expr, "Lists are not allowed"))?;

                    if punc.chars().count() == 1 {
                        let c = punc.chars().next().unwrap(); // checked count above
                        if let Some(out) = user_cfg_char_to_output.get(&c) {
                            if out.len() > 1 {
                                bail_expr!(
                                    punc_expr,
                                    "This character is a single-output with multiple keys\n
                                       and is not yet supported as use for punctuation."
                                );
                            }
                            puncs.push(out[0]);
                            return Ok(puncs);
                        }
                    }

                    let osc = str_to_oscode(punc)
                        .ok_or_else(|| anyhow_expr!(&punc_expr, "Unknown key name"))?;
                    puncs.push(ZchOutput::Lowercase(osc));

                    Ok(puncs)
                })?
                .into_iter()
                .collect();
            config.zch_cfg_smart_space_punctuation.shrink_to_fit();
        }

        // process zippy file
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
                                if let Some(out) = user_cfg_char_to_output.get(&out_char) {
                                    zch_output.extend(out.iter());
                                    return Ok(zch_output);
                                }

                                let out_key = out_char.to_lowercase().next().unwrap();
                                let key_name = out_key.encode_utf8(&mut char_buf);
                                let osc = match key_name as &str {
                                    " " => OsCode::KEY_SPACE,
                                    _ => str_to_oscode(key_name).ok_or_else(|| {
                                        anyhow_expr!(
                                            &exprs[1],
                                            "Unknown output key name '{}':\n{}: {line}",
                                            out_char,
                                            line_number + 1,
                                        )
                                    })?,
                                };
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
                        (chord_chars, input_left_to_parse) =
                            match input_left_to_parse.split_once(' ') {
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
                                let map =
                                    Arc::new(Mutex::new(ZchPossibleChords(SubsetMap::ssm_new())));
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
        Ok((
            Arc::into_inner(res).expect("no other refs").into_inner(),
            config,
        ))
    }

    fn parse_single_zippy_output_mapping(
        output: &str,
        output_expr: &SExpr,
        is_noerase: bool,
    ) -> Result<ZchOutput> {
        let (output_mods, output_key) = parse_mod_prefix(output)?;
        if output_mods.contains(&KeyCode::LShift) && output_mods.contains(&KeyCode::RShift) {
            bail_expr!(
                output_expr,
                "Both shifts are used which is redundant, use only one."
            );
        }
        if output_mods
            .iter()
            .any(|m| !matches!(m, KeyCode::LShift | KeyCode::RShift | KeyCode::RAlt))
        {
            bail_expr!(output_expr, "Only S- and AG- are supported.");
        }
        let output_osc = str_to_oscode(output_key)
            .ok_or_else(|| anyhow_expr!(output_expr, "unknown key name"))?;
        let output = match output_mods.len() {
            0 => match is_noerase {
                false => ZchOutput::Lowercase(output_osc),
                true => ZchOutput::NoEraseLowercase(output_osc),
            },
            1 => match output_mods[0] {
                KeyCode::LShift | KeyCode::RShift => match is_noerase {
                    false => ZchOutput::Uppercase(output_osc),
                    true => ZchOutput::NoEraseUppercase(output_osc),
                },
                KeyCode::RAlt => match is_noerase {
                    false => ZchOutput::AltGr(output_osc),
                    true => ZchOutput::NoEraseAltGr(output_osc),
                },
                _ => unreachable!("forbidden by earlier parsing"),
            },
            2 => match is_noerase {
                false => ZchOutput::ShiftAltGr(output_osc),
                true => ZchOutput::NoEraseShiftAltGr(output_osc),
            },
            _ => {
                unreachable!("contains at most: altgr and one of the shifts")
            }
        };
        Ok(output)
    }
}
