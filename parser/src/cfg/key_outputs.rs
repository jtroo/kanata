use super::*;

// Note: this uses a Vec inside the HashMap instead of a HashSet because ordering matters, e.g. for
// chords like `S-b`, we want to ensure that `b` is checked first because key repeat for `b` is
// useful while it is not useful for shift. The outputs should be iterated over in reverse order.
pub type KeyOutputs = Vec<HashMap<OsCode, Vec<OsCode>>>;

/// Creates a `KeyOutputs` from `layers::LAYERS`.
pub(crate) fn create_key_outputs(
    layers: &KLayers,
    overrides: &Overrides,
    chords_v2: &Option<ChordsV2<'static, KanataCustom>>,
) -> KeyOutputs {
    let mut outs = KeyOutputs::new();
    for (layer_idx, layer) in layers.iter().enumerate() {
        let mut layer_outputs = HashMap::default();
        for (i, action) in layer[0].iter().enumerate() {
            let osc_slot = match i.try_into() {
                Ok(i) => i,
                Err(_) => continue,
            };
            add_key_output_from_action_to_key_pos(osc_slot, action, &mut layer_outputs, overrides);
            add_chordsv2_output_for_key_pos(
                osc_slot,
                layer_idx,
                chords_v2,
                &mut layer_outputs,
                overrides,
            );
        }
        outs.push(layer_outputs);
    }
    for layer_outs in outs.iter_mut() {
        for keys_out in layer_outs.values_mut() {
            keys_out.shrink_to_fit();
        }
        layer_outs.shrink_to_fit();
    }
    outs.shrink_to_fit();
    outs
}

pub(crate) fn add_chordsv2_output_for_key_pos(
    osc_slot: OsCode,
    layer_idx: usize,
    chords_v2: &Option<ChordsV2<'static, KanataCustom>>,
    outputs: &mut HashMap<OsCode, Vec<OsCode>>,
    overrides: &Overrides,
) {
    assert!(layer_idx <= usize::from(u16::MAX));
    let Some(chords_v2) = chords_v2.as_ref() else {
        return;
    };
    let Some(chords_for_key) = chords_v2.chords().mapping.get(&u16::from(osc_slot)) else {
        return;
    };
    for chord in chords_for_key.chords.iter() {
        if !chord.disabled_layers.contains(&(layer_idx as u16)) {
            add_key_output_from_action_to_key_pos(osc_slot, chord.action, outputs, overrides);
        }
    }
}

pub(crate) fn add_key_output_from_action_to_key_pos(
    osc_slot: OsCode,
    action: &KanataAction,
    outputs: &mut HashMap<OsCode, Vec<OsCode>>,
    overrides: &Overrides,
) {
    match action {
        Action::KeyCode(kc) => {
            add_kc_output(osc_slot, kc.into(), outputs, overrides);
        }
        Action::HoldTap(HoldTapAction {
            tap,
            hold,
            timeout_action,
            ..
        }) => {
            add_key_output_from_action_to_key_pos(osc_slot, tap, outputs, overrides);
            add_key_output_from_action_to_key_pos(osc_slot, hold, outputs, overrides);
            add_key_output_from_action_to_key_pos(osc_slot, timeout_action, outputs, overrides);
        }
        Action::OneShot(OneShot { action: ac, .. }) => {
            add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
        }
        Action::MultipleKeyCodes(kcs) => {
            for kc in kcs.iter() {
                add_kc_output(osc_slot, kc.into(), outputs, overrides);
            }
        }
        Action::MultipleActions(actions) => {
            for ac in actions.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
            }
        }
        Action::TapDance(TapDance { actions, .. }) => {
            for ac in actions.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
            }
        }
        Action::Fork(ForkConfig { left, right, .. }) => {
            add_key_output_from_action_to_key_pos(osc_slot, left, outputs, overrides);
            add_key_output_from_action_to_key_pos(osc_slot, right, outputs, overrides);
        }
        Action::Chords(ChordsGroup { chords, .. }) => {
            for (_, ac) in chords.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
            }
        }
        Action::Switch(Switch { cases }) => {
            for case in cases.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, case.1, outputs, overrides);
            }
        }
        Action::Custom(cacs) => {
            for ac in cacs.iter() {
                match ac {
                    CustomAction::Unmodded { keys } | CustomAction::Unshifted { keys } => {
                        for k in keys.iter() {
                            add_kc_output(osc_slot, k.into(), outputs, overrides);
                        }
                    }
                    _ => {}
                }
            }
        }
        Action::NoOp
        | Action::Trans
        | Action::Repeat
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::RepeatableSequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_) => {}
    };
}

pub(crate) fn add_kc_output(
    osc_slot: OsCode,
    osc: OsCode,
    outs: &mut HashMap<OsCode, Vec<OsCode>>,
    overrides: &Overrides,
) {
    let outputs = match outs.entry(osc_slot) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => v.insert(vec![]),
    };
    if !outputs.contains(&osc) {
        outputs.push(osc);
    }
    for ov_osc in overrides
        .output_non_mods_for_input_non_mod(osc)
        .iter()
        .copied()
    {
        if !outputs.contains(&ov_osc) {
            outputs.push(ov_osc);
        }
    }
}
