use itertools::Itertools;
use kanata_keyberon::chord::{ChordV2, ChordsForKey, ChordsForKeys, ReleaseBehaviour};
use rustc_hash::{FxHashMap, FxHashSet};

use std::fs;

use crate::{anyhow_expr, bail_expr};

use super::*;

pub(crate) fn parse_defchordv2(
    exprs: &[SExpr],
    s: &ParserState,
) -> Result<ChordsForKeys<'static, KanataCustom>> {
    if exprs[0].atom(None).expect("should be atom") == "defchordsv2-experimental" {
        log::warn!(
            "You should replace defchordsv2-experimental with defchordsv2.\n\
             Using -experimental will be invalid in the future."
        );
    }

    let mut chunks = exprs[1..].chunks_exact(5);
    let mut chords_container = ChordsForKeys::<'static, KanataCustom> {
        mapping: FxHashMap::default(),
    };

    let mut all_participating_key_sets = FxHashSet::default();

    let all_chords = chunks
        .by_ref()
        .flat_map(|chunk| match chunk[0] {
            // Match a line like
            // (include filename.txt) () 100 all-released (layer1 layer2)
            SExpr::List(Spanned {
                t: ref exprs,
                span: _,
            }) if matches!(exprs.first(), Some(SExpr::Atom(a)) if a.t == "include") => {
                let file_name = exprs[1].atom(s.vars()).unwrap();
                let chord_translation = ChordTranslation::create(
                    file_name,
                    &chunk[2],
                    &chunk[3],
                    &chunk[4],
                    &s.layers[0][0],
                );
                let chord_definitions = parse_chord_file(file_name).unwrap();
                let processed = chord_definitions.iter().map(|chord_def| {
                    let chunk = chord_translation.translate_chord(chord_def);
                    parse_single_chord(&chunk, s, &mut all_participating_key_sets)
                });
                Ok::<_, ParseError>(processed.collect_vec())
            }
            _ => Ok(vec![parse_single_chord(
                chunk,
                s,
                &mut all_participating_key_sets,
            )]),
        })
        .flat_map(|vec_result| vec_result.into_iter())
        .collect::<Vec<Result<_>>>();
    let unsuccessful = all_chords
        .iter()
        .filter_map(|r| r.as_ref().err())
        .collect::<Vec<_>>();
    if let Some(e) = unsuccessful.first() {
        return Err((*e).clone());
    }

    let successful = all_chords.into_iter().filter_map(Result::ok).collect_vec();
    for chord in successful {
        for pkey in chord.participating_keys.iter().copied() {
            //log::trace!("chord for key:{pkey:?} > {chord:?}");
            chords_container
                .mapping
                .entry(pkey)
                .or_insert(ChordsForKey { chords: vec![] })
                .chords
                .push(s.a.sref(chord.clone()));
        }
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        bail_expr!(
            rem.last().unwrap(),
            "Incomplete chord entry. Each chord entry must have 5 items:\n\
        participating-keys, action, timeout, release-type, disabled-layers"
        );
    }
    Ok(chords_container)
}

fn parse_single_chord(
    chunk: &[SExpr],
    s: &ParserState,
    all_participating_key_sets: &mut FxHashSet<Vec<u16>>,
) -> Result<ChordV2<'static, KanataCustom>> {
    let participants = parse_participating_keys(&chunk[0], s)?;
    if !all_participating_key_sets.insert(participants.clone()) {
        bail_expr!(
            &chunk[0],
            "Duplicate participating-keys, key sets may be used only once."
        );
    }
    let action = parse_action(&chunk[1], s)?;
    let timeout = parse_timeout(&chunk[2], s)?;
    let release_behaviour = parse_release_behaviour(&chunk[3], s)?;
    let disabled_layers = parse_disabled_layers(&chunk[4], s)?;
    let chord: ChordV2<'static, KanataCustom> = ChordV2 {
        action,
        participating_keys: s.a.sref_vec(participants.clone()),
        pending_duration: timeout,
        disabled_layers: s.a.sref_vec(disabled_layers),
        release_behaviour,
    };
    Ok(s.a.sref(chord).clone())
}

fn parse_participating_keys(keys: &SExpr, s: &ParserState) -> Result<Vec<u16>> {
    let mut participants = keys
        .list(s.vars())
        .map(|l| {
            l.iter()
                .try_fold(vec![], |mut keys, key| -> Result<Vec<u16>> {
                    let k = key.atom(s.vars()).and_then(str_to_oscode).ok_or_else(|| {
                        anyhow_expr!(
                            key,
                            "The first chord item must be a list of keys.\nInvalid key name."
                        )
                    })?;
                    keys.push(k.into());
                    Ok(keys)
                })
        })
        .ok_or_else(|| anyhow_expr!(keys, "The first chord item must be a list of keys."))??;
    if participants.len() < 2 {
        bail_expr!(keys, "The minimum number of participating chord keys is 2");
    }
    participants.sort();
    Ok(participants)
}

fn parse_timeout(chunk: &SExpr, s: &ParserState) -> Result<u16> {
    let timeout = parse_non_zero_u16(chunk, s, "chord timeout")?;
    Ok(timeout)
}

fn parse_release_behaviour(
    release_behaviour_string: &SExpr,
    s: &ParserState,
) -> Result<ReleaseBehaviour> {
    let release_behaviour = release_behaviour_string
        .atom(s.vars())
        .and_then(|r| {
            Some(match r {
                "first-release" => ReleaseBehaviour::OnFirstRelease,
                "all-released" => ReleaseBehaviour::OnLastRelease,
                _ => return None,
            })
        })
        .ok_or_else(|| {
            anyhow_expr!(
                release_behaviour_string,
                "Chord release behaviour must be one of:\n\
                first-release | all-released"
            )
        })?;
    Ok(release_behaviour)
}

fn parse_disabled_layers(disabled_layers: &SExpr, s: &ParserState) -> Result<Vec<u16>> {
    let disabled_layers = disabled_layers
        .list(s.vars())
        .map(|dl| {
            dl.iter()
                .try_fold(vec![], |mut layers, layer| -> Result<Vec<u16>> {
                    let l_idx = layer
                        .atom(s.vars())
                        .and_then(|l| s.layer_idxs.get(l))
                        .ok_or_else(|| anyhow_expr!(layer, "Not a known layer name."))?;
                    layers.push((*l_idx) as u16);
                    Ok(layers)
                })
        })
        .ok_or_else(|| {
            anyhow_expr!(
                disabled_layers,
                "Disabled layers must be a list of layer names"
            )
        })??;
    Ok(disabled_layers)
}

fn parse_chord_file(file_name: &str) -> Result<Vec<ChordDefinition>> {
    let input_data =
        fs::read_to_string(file_name).unwrap_or_else(|_| panic!("Unable to read file {file_name}"));
    let parsed_chords = parse_input(&input_data).unwrap();
    Ok(parsed_chords)
}

fn parse_input(input: &str) -> Result<Vec<ChordDefinition>> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
        .map(|line| {
            let mut caps = line.split('\t');
            let error_message = format!(
                "Each line needs to have an action separated by a tab character, got '{line}'"
            );
            let keys = caps.next().expect(&error_message);
            let action = caps.next().expect(&error_message);
            Ok(ChordDefinition {
                keys: keys.to_string(),
                action: action.to_string(),
            })
        })
        .collect()
}

#[derive(Debug)]
struct ChordDefinition {
    keys: String,
    action: String,
}

struct ChordTranslation<'a> {
    file_name: &'a str,
    target_map: FxHashMap<String, String>,
    postprocess_map: FxHashMap<String, String>,
    timeout: &'a SExpr,
    release_behaviour: &'a SExpr,
    disabled_layers: &'a SExpr,
}

impl<'a> ChordTranslation<'a> {
    fn create(
        file_name: &'a str,
        timeout: &'a SExpr,
        release_behaviour: &'a SExpr,
        disabled_layers: &'a SExpr,
        first_layer: &[Action<'static, &&[&CustomAction]>],
    ) -> Self {
        let postprocess_map: FxHashMap<String, String> = [
            ("semicolon", ";"),
            ("colon", "S-."),
            ("slash", "/"),
            ("apostrophe", "'"),
            ("dot", "."),
            (" ", "spc"),
        ]
        .iter()
        .cloned()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let target_map = first_layer
            .iter()
            .enumerate()
            .filter_map(|(idx, layout)| {
                layout
                    .key_codes()
                    .next()
                    .map(|kc| kc.to_string().to_lowercase())
                    .zip(
                        idx.try_into()
                            .ok()
                            .and_then(OsCode::from_u16)
                            .map(|osc| osc.to_string().to_lowercase()),
                    )
            })
            .collect::<Vec<_>>()
            .into_iter()
            .chain(vec![(" ".to_string(), "spc".to_string())])
            .collect::<FxHashMap<_, _>>();
        ChordTranslation {
            file_name,
            target_map,
            postprocess_map,
            timeout,
            release_behaviour,
            disabled_layers,
        }
    }

    fn post_process(&self, converted: &str) -> String {
        self.postprocess_map
            .get(converted)
            .map(|c| c.to_string())
            .unwrap_or_else(|| {
                if converted.chars().all(|c| c.is_uppercase()) {
                    format!("S-{}", converted.to_lowercase())
                } else {
                    converted.to_string()
                }
            })
    }

    fn participant_keys(&self, keys: &str) -> Vec<String> {
        keys.chars()
            .map(|key| {
                self.target_map
                    .get(key.to_string().to_lowercase().as_str())
                    .map(|c| self.postprocess_map.get(c).unwrap_or(c).to_string())
                    .unwrap_or_else(|| key.to_string())
            })
            .collect::<Vec<String>>()
    }

    fn action(&self, action: &str) -> Vec<String> {
        let mut action_strings = action
            .chars()
            .map(|c| self.post_process(&c.to_string()))
            .collect_vec();
        // Wait 50ms for one-shot Shift to release
        // TODO: This would be better handled by a (multi (release-key lsft)(release-key rsft))
        // but I haven't gotten that to work yet.
        action_strings.insert(1, "50".to_string());
        action_strings.extend_from_slice(&[
            "sldr".to_string(),
            "spc".to_string(),
            "nop0".to_string(),
        ]);
        action_strings
    }

    fn translate_chord(&self, chord_def: &ChordDefinition) -> Vec<SExpr> {
        let sexpr_string = format!(
            "(({}) (macro {}))",
            self.participant_keys(&chord_def.keys).join(" "),
            self.action(&chord_def.action).join(" ")
        );
        let mut participant_action = sexpr::parse(&sexpr_string, self.file_name).unwrap()[0]
            .t
            .clone();
        participant_action.extend_from_slice(&[
            self.timeout.clone(),
            self.release_behaviour.clone(),
            self.disabled_layers.clone(),
        ]);
        participant_action
    }
}
