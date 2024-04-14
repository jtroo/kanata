use kanata_keyberon::chord::{ChordV2, ChordsForKey, ChordsForKeys, ReleaseBehaviour};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{anyhow_expr, bail_expr};

use super::*;

pub(crate) fn parse_defchordv2(
    exprs: &[SExpr],
    s: &ParserState,
) -> Result<ChordsForKeys<'static, KanataCustom>> {
    let mut chunks = exprs[1..].chunks_exact(5);
    let mut all_chords = FxHashSet::default();
    let mut chords_container = ChordsForKeys::<'static, KanataCustom> {
        mapping: FxHashMap::default(),
    };
    for chunk in chunks.by_ref() {
        let keys = &chunk[0];

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
        if !all_chords.insert(participants.clone()) {
            bail_expr!(
                keys,
                "This chord has previously been defined.\n\
                Only one set of chords must exist for one key combination."
            );
        }

        let action = parse_action(&chunk[1], s)?;
        let timeout = parse_non_zero_u16(&chunk[2], s, "chord timeout")?;
        let release_behaviour = chunk[3]
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
                    &chunk[3],
                    "Chord release behaviour must be one of:\n\
                first-release | all-released"
                )
            })?;

        let disabled_layers = &chunk[4];
        let disabled_layers = disabled_layers
            .list(s.vars())
            .map(|dl| {
                dl.iter()
                    .try_fold(vec![], |mut layers, layer| -> Result<Vec<u16>> {
                        let l_idx = layer
                            .atom(s.vars())
                            .and_then(|l| s.layer_idxs.get(l))
                            .ok_or_else(|| anyhow_expr!(layer, "Not a known layer name."))?;
                        layers.push((*l_idx * 2) as u16);
                        layers.push((*l_idx * 2 + 1) as u16);
                        Ok(layers)
                    })
            })
            .ok_or_else(|| {
                anyhow_expr!(
                    disabled_layers,
                    "Disabled layers must be a list of layer names"
                )
            })??;
        let chord = ChordV2 {
            action,
            participating_keys: s.a.sref_vec(participants.clone()),
            pending_duration: timeout,
            disabled_layers: s.a.sref_vec(disabled_layers),
            release_behaviour,
        };
        let chord = s.a.sref(chord);
        for pkey in participants.iter().copied() {
            log::trace!("chord for key:{pkey:?} > {chord:?}");
            chords_container
                .mapping
                .entry(pkey)
                .or_insert(ChordsForKey { chords: vec![] })
                .chords
                .push(chord);
        }
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        bail_expr!(
            rem.last().unwrap(),
            "Incomplete chord entry. Each chord entry must have 5 items:\n\
        particpating-keys, action, timeout, release-type, disabled-layers"
        );
    }
    Ok(chords_container)
}
