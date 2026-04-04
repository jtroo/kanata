use super::*;

use crate::anyhow_expr;
use crate::anyhow_span;
use crate::bail;
use crate::bail_expr;
use crate::bail_span;
use crate::err_expr;

#[derive(Debug, Clone)]
pub(crate) struct ChordGroup {
    id: u16,
    name: String,
    keys: Vec<String>,
    coords: Vec<((u8, u16), ChordKeys)>,
    chords: HashMap<u128, SExpr>,
    timeout: u16,
}

pub(crate) fn parse_chord(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "Action chord expects a chords group name followed by an identifier";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let name = ac_params[0]
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "{ERR_MSG}"))?;
    let group = match s.chord_groups.get(name) {
        Some(t) => t,
        None => bail_expr!(&ac_params[0], "Referenced unknown chord group: {}.", name),
    };
    let chord_key_index = ac_params[1]
        .atom(s.vars())
        .map(|s| match group.keys.iter().position(|e| e == s) {
            Some(i) => Ok(i),
            None => err_expr!(
                &ac_params[1],
                r#"Identifier "{}" is not used in chord group "{}"."#,
                &s,
                name,
            ),
        })
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "{ERR_MSG}"))??;
    let chord_keys: u128 = 1 << chord_key_index;

    // We don't yet know at this point what the entire chords group will look like nor at which
    // coords this action will end up. So instead we store a dummy action which will be properly
    // resolved in `resolve_chord_groups`.
    Ok(s.a.sref(Action::Chords(s.a.sref(ChordsGroup {
        timeout: group.timeout,
        coords: s.a.sref_vec(vec![((0, group.id), chord_keys)]),
        chords: s.a.sref_vec(vec![]),
    }))))
}

pub(crate) fn parse_chord_groups(
    exprs: &[&Spanned<Vec<SExpr>>],
    s: &mut ParserState,
) -> Result<()> {
    const MSG: &str = "Incorrect number of elements found in defchords.\nThere should be the group name, followed by timeout, followed by keys-action pairs";
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.t.iter(), "defchords")?;
        let name = subexprs
            .next()
            .and_then(|e| e.atom(s.vars()))
            .ok_or_else(|| anyhow_span!(expr, "{MSG}"))?
            .to_owned();
        let timeout = match subexprs.next() {
            Some(e) => parse_non_zero_u16(e, s, "timeout")?,
            None => bail_span!(expr, "{MSG}"),
        };
        let id = match s.chord_groups.len().try_into() {
            Ok(id) => id,
            Err(_) => bail_span!(expr, "Maximum number of chord groups exceeded."),
        };
        let mut group = ChordGroup {
            id,
            name: name.clone(),
            keys: Vec::new(),
            coords: Vec::new(),
            chords: HashMap::default(),
            timeout,
        };
        // Read k-v pairs from the configuration
        while let Some(keys_expr) = subexprs.next() {
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail_expr!(
                    keys_expr,
                    "Key list found without action - add an action for this chord"
                ),
            };
            let mut keys = keys_expr
                .list(s.vars())
                .map(|keys| {
                    keys.iter().map(|key| {
                        key.atom(s.vars()).ok_or_else(|| {
                            anyhow_expr!(
                                key,
                                "Chord keys cannot be lists. Invalid key name: {:?}",
                                key
                            )
                        })
                    })
                })
                .ok_or_else(|| anyhow_expr!(keys_expr, "Chord must be a list/set of keys"))?;
            let mask: u128 = keys.try_fold(0, |mask, key| {
                let key = key?;
                let index = match group.keys.iter().position(|k| k == key) {
                    Some(i) => i,
                    None => {
                        let i = group.keys.len();
                        if i + 1 > MAX_CHORD_KEYS {
                            bail_expr!(keys_expr, "Maximum number of keys in a chords group ({MAX_CHORD_KEYS}) exceeded - found {}", i + 1);
                        }
                        group.keys.push(key.to_owned());
                        i
                    }
                };
                Ok(mask | (1 << index))
            })?;
            if group.chords.insert(mask, action.clone()).is_some() {
                bail_expr!(keys_expr, "Duplicate chord in group {name}");
            }
        }
        if s.chord_groups.insert(name.to_owned(), group).is_some() {
            bail_span!(expr, "Duplicate chords group: {}", name);
        }
    }
    Ok(())
}

pub(crate) fn resolve_chord_groups(layers: &mut IntermediateLayers, s: &ParserState) -> Result<()> {
    let mut chord_groups = s.chord_groups.values().cloned().collect::<Vec<_>>();
    chord_groups.sort_by_key(|group| group.id);

    for layer in layers.iter() {
        for (i, row) in layer.iter().enumerate() {
            for (j, cell) in row.iter().enumerate() {
                find_chords_coords(&mut chord_groups, (i as u8, j as u16), cell);
            }
        }
    }

    let chord_groups = chord_groups.into_iter().map(|group| {
        // Check that all keys in the chord group have been assigned to some coordinate
        for (key_index, key) in group.keys.iter().enumerate() {
            let key_mask = 1 << key_index;
            if !group.coords.iter().any(|(_, keys)| keys & key_mask != 0) {
                bail!("coord group `{0}` defines unused key `{1}`, did you forget to bind `(chord {0} {1})`?", group.name, key)
            }
        }

        let chords = group.chords.iter().map(|(mask, action)| {
            Ok((*mask, parse_action(action, s)?))
        }).collect::<Result<Vec<_>>>()?;

        Ok(s.a.sref(ChordsGroup {
            coords: s.a.sref_vec(group.coords),
            chords: s.a.sref_vec(chords),
            timeout: group.timeout,
        }))
    }).collect::<Result<Vec<_>>>()?;

    for layer in layers.iter_mut() {
        for row in layer.iter_mut() {
            for cell in row.iter_mut() {
                if let Some(action) = fill_chords(&chord_groups, cell, s) {
                    *cell = action;
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn find_chords_coords(
    chord_groups: &mut [ChordGroup],
    coord: (u8, u16),
    action: &KanataAction,
) {
    match action {
        Action::Chords(ChordsGroup { coords, .. }) => {
            for ((_, group_id), chord_keys) in coords.iter() {
                let group = &mut chord_groups[*group_id as usize];
                group.coords.push((coord, *chord_keys));
            }
        }
        Action::NoOp
        | Action::Trans
        | Action::Src
        | Action::Repeat
        | Action::KeyCode(_)
        | Action::MultipleKeyCodes(_)
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::RepeatableSequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_)
        | Action::OneShotIgnoreEventsTicks(_)
        | Action::Custom(_) => {}
        Action::HoldTap(HoldTapAction { tap, hold, .. }) => {
            find_chords_coords(chord_groups, coord, tap);
            find_chords_coords(chord_groups, coord, hold);
        }
        Action::OneShot(OneShot { action: ac, .. }) => {
            find_chords_coords(chord_groups, coord, ac);
        }
        Action::MultipleActions(actions) => {
            for ac in actions.iter() {
                find_chords_coords(chord_groups, coord, ac);
            }
        }
        Action::TapDance(TapDance { actions, .. }) => {
            for ac in actions.iter() {
                find_chords_coords(chord_groups, coord, ac);
            }
        }
        Action::Fork(ForkConfig { left, right, .. }) => {
            find_chords_coords(chord_groups, coord, left);
            find_chords_coords(chord_groups, coord, right);
        }
        Action::Switch(Switch { cases }) => {
            for case in cases.iter() {
                find_chords_coords(chord_groups, coord, case.1);
            }
        }
    }
}

pub(crate) fn fill_chords(
    chord_groups: &[&'static ChordsGroup<KanataCustom>],
    action: &KanataAction,
    s: &ParserState,
) -> Option<KanataAction> {
    match action {
        Action::Chords(ChordsGroup { coords, .. }) => {
            let ((_, group_id), _) = coords
                .iter()
                .next()
                .expect("unresolved chords should have exactly one entry");
            Some(Action::Chords(chord_groups[*group_id as usize]))
        }
        Action::NoOp
        | Action::Trans
        | Action::Repeat
        | Action::Src
        | Action::KeyCode(_)
        | Action::MultipleKeyCodes(_)
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::RepeatableSequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_)
        | Action::OneShotIgnoreEventsTicks(_)
        | Action::Custom(_) => None,
        Action::HoldTap(&hta @ HoldTapAction { tap, hold, .. }) => {
            let new_tap = fill_chords(chord_groups, &tap, s);
            let new_hold = fill_chords(chord_groups, &hold, s);
            if new_tap.is_some() || new_hold.is_some() {
                Some(Action::HoldTap(s.a.sref(HoldTapAction {
                    hold: new_hold.unwrap_or(hold),
                    tap: new_tap.unwrap_or(tap),
                    ..hta
                })))
            } else {
                None
            }
        }
        Action::OneShot(&os @ OneShot { action: ac, .. }) => {
            fill_chords(chord_groups, ac, s).map(|ac| {
                Action::OneShot(s.a.sref(OneShot {
                    action: s.a.sref(ac),
                    ..os
                }))
            })
        }
        Action::MultipleActions(actions) => {
            let new_actions = actions
                .iter()
                .map(|ac| fill_chords(chord_groups, ac, s))
                .collect::<Vec<_>>();
            if new_actions.iter().any(|it| it.is_some()) {
                let new_actions = new_actions
                    .iter()
                    .zip(**actions)
                    .map(|(new_ac, ac)| new_ac.unwrap_or(*ac))
                    .collect::<Vec<_>>();
                Some(Action::MultipleActions(s.a.sref(s.a.sref_vec(new_actions))))
            } else {
                None
            }
        }
        Action::TapDance(&td @ TapDance { actions, .. }) => {
            let new_actions = actions
                .iter()
                .map(|ac| fill_chords(chord_groups, ac, s))
                .collect::<Vec<_>>();
            if new_actions.iter().any(|it| it.is_some()) {
                let new_actions = new_actions
                    .iter()
                    .zip(actions)
                    .map(|(new_ac, ac)| new_ac.map(|v| s.a.sref(v)).unwrap_or(*ac))
                    .collect::<Vec<_>>();
                Some(Action::TapDance(s.a.sref(TapDance {
                    actions: s.a.sref_vec(new_actions),
                    ..td
                })))
            } else {
                None
            }
        }
        Action::Fork(&fcfg @ ForkConfig { left, right, .. }) => {
            let new_left = fill_chords(chord_groups, &left, s);
            let new_right = fill_chords(chord_groups, &right, s);
            if new_left.is_some() || new_right.is_some() {
                Some(Action::Fork(s.a.sref(ForkConfig {
                    left: new_left.unwrap_or(left),
                    right: new_right.unwrap_or(right),
                    ..fcfg
                })))
            } else {
                None
            }
        }
        Action::Switch(Switch { cases }) => {
            let mut new_cases = vec![];
            for case in cases.iter() {
                new_cases.push((
                    case.0,
                    fill_chords(chord_groups, case.1, s)
                        .map(|ac| s.a.sref(ac))
                        .unwrap_or(case.1),
                    case.2,
                ));
            }
            Some(Action::Switch(s.a.sref(Switch {
                cases: s.a.sref_vec(new_cases),
            })))
        }
    }
}
