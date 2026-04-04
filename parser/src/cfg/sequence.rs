use super::*;

use crate::anyhow_expr;
use crate::bail;
use crate::bail_expr;

const SEQ_ERR: &str = "defseq expects pairs of parameters: <virtual_key_name> <key_list>";

pub(crate) fn parse_sequence_start(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str =
        "sequence expects one or two params: <timeout-override> <?input-mode-override>";
    if !matches!(ac_params.len(), 1 | 2) {
        bail!("{ERR_MSG}\nfound {} items", ac_params.len());
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout-override")?;
    let input_mode = if ac_params.len() > 1 {
        if let Some(Ok(input_mode)) = ac_params[1]
            .atom(s.vars())
            .map(SequenceInputMode::try_from_str)
        {
            input_mode
        } else {
            bail_expr!(&ac_params[1], "{ERR_MSG}\n{}", SequenceInputMode::err_msg());
        }
    } else {
        s.default_sequence_input_mode
    };
    custom(CustomAction::SequenceLeader(timeout, input_mode), &s.a)
}

pub(crate) fn parse_sequence_noerase(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "sequence-noerase expects one: <noerase-count>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}\nfound {} items", ac_params.len());
    }
    let count = parse_non_zero_u16(&ac_params[0], s, "noerase-count")?;
    custom(CustomAction::SequenceNoerase(count), &s.a)
}

pub(crate) fn parse_sequences(exprs: &[&Vec<SExpr>], s: &ParserState) -> Result<KeySeqsToFKeys> {
    let mut sequences = Trie::new();
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defseq")?.peekable();

        while let Some(vkey_expr) = subexprs.next() {
            let vkey = vkey_expr.atom(s.vars()).ok_or_else(|| {
                anyhow_expr!(vkey_expr, "{SEQ_ERR}\nvirtual_key_name must not be a list")
            })?;
            #[cfg(feature = "lsp")]
            s.lsp_hints
                .borrow_mut()
                .reference_locations
                .virtual_key
                .push(vkey, vkey_expr.span());
            if !s.virtual_keys.contains_key(vkey) {
                bail_expr!(
                    vkey_expr,
                    "{SEQ_ERR}\nThe referenced key does not exist: {vkey}"
                );
            }
            let key_seq_expr = subexprs
                .next()
                .ok_or_else(|| anyhow_expr!(vkey_expr, "{SEQ_ERR}\nMissing key_list for {vkey}"))?;
            let key_seq = key_seq_expr.list(s.vars()).ok_or_else(|| {
                anyhow_expr!(key_seq_expr, "{SEQ_ERR}\nGot a non-list for key_list")
            })?;
            if key_seq.is_empty() {
                bail_expr!(key_seq_expr, "{SEQ_ERR}\nkey_list cannot be empty");
            }

            let keycode_seq = parse_sequence_keys(key_seq, s)?;

            // Generate permutations of sequences for overlapping keys.
            let mut permutations = vec![vec![]];
            let mut vals = keycode_seq.iter().copied();
            while let Some(val) = vals.next() {
                if val & KEY_OVERLAP_MARKER == 0 {
                    for p in permutations.iter_mut() {
                        p.push(val);
                    }
                    continue;
                }

                if val == 0x0400 {
                    bail_expr!(
                        key_seq_expr,
                        "O-(...) lists must have a minimum of 2 elements"
                    );
                }
                let mut values_to_permute = vec![val];
                for val in vals.by_ref() {
                    if val == 0x0400 {
                        break;
                    }
                    values_to_permute.push(val);
                }

                let ps = match values_to_permute.len() {
                    0 | 1 => bail_expr!(
                        key_seq_expr,
                        "O-(...) lists must have a minimum of 2 elements"
                    ),
                    2..=6 => gen_permutations(&values_to_permute[..]),
                    _ => bail_expr!(
                        key_seq_expr,
                        "O-(...) lists must have a maximum of 6 elements"
                    ),
                };

                let mut new_permutations: Vec<Vec<u16>> = vec![];
                for p in permutations.iter() {
                    for p2 in ps.iter() {
                        new_permutations.push(
                            p.iter()
                                .copied()
                                .chain(p2.iter().copied().chain([KEY_OVERLAP_MARKER]))
                                .collect(),
                        );
                    }
                }
                permutations = new_permutations;
            }

            for p in permutations.into_iter() {
                if sequences.ancestor_exists(&p) {
                    bail_expr!(
                        key_seq_expr,
                        "Sequence has a conflict: its sequence contains an earlier defined sequence"
                    );
                }
                if sequences.descendant_exists(&p) {
                    bail_expr!(
                        key_seq_expr,
                        "Sequence has a conflict: its sequence is contained within an earlier defined seqence"
                    );
                }
                sequences.insert(
                    p,
                    s.virtual_keys
                        .get(vkey)
                        .map(|(y, _)| get_fake_key_coords(*y))
                        .expect("vk exists, checked earlier"),
                );
            }
        }
    }
    Ok(sequences)
}

pub(crate) fn parse_sequence_keys(exprs: &[SExpr], s: &ParserState) -> Result<Vec<u16>> {
    use SequenceEvent::*;

    // Reuse macro parsing but do some other processing since sequences don't support everything
    // that can go in a macro, and also change error messages of course.
    let mut exprs_remaining = exprs;
    let mut all_keys = Vec::new();
    while !exprs_remaining.is_empty() {
        let (mut keys, exprs_remaining_tmp) =
            match parse_macro_item_impl(exprs_remaining, s, MacroNumberParseMode::Action) {
                Ok(res) => {
                    if res.0.iter().any(|k| !matches!(k, Press(..) | Release(..))) {
                        // Determine the bad expression depending on how many expressions were consumed
                        // by parse_macro_item_impl.
                        let bad_expr = if exprs_remaining.len() - res.1.len() == 1 {
                            &exprs_remaining[0]
                        } else {
                            // This error message will have an imprecise span since it will take the
                            // whole chorded list instead of the single element inside that's not a
                            // standard key. Oh well, should still be helpful. I'm too lazy to write
                            // the code to find the exact expr to use right now.
                            &exprs_remaining[1]
                        };
                        bail_expr!(bad_expr, "{SEQ_ERR}\nFound invalid key/chord in key_list");
                    }

                    // The keys are currenty in the form of SequenceEvent::{Press, Release}. This is
                    // not what we want.
                    //
                    // The trivial and incorrect way to parse this would be to just take all of the
                    // presses. However, we need to transform chorded keys/lists like S-a or S-(a b) to
                    // have the upper bits set, to be able to differentiate (S-a b) from (S-(a b)).
                    //
                    // The order of presses and releases reveals whether or not a key is chorded with
                    // some modifier. When a chord starts, there are multiple presses in a row, whereas
                    // non-chords will always be a press followed by a release. Likewise, a chord
                    // ending is marked by multiple releases in a row.
                    let mut mods_currently_held = vec![];
                    let mut key_actions = res.0.iter().peekable();
                    let mut seq = vec![];
                    let mut do_release_mod = false;
                    while let Some(action) = key_actions.next() {
                        match action {
                            Press(pressed) => {
                                if matches!(key_actions.peek(), Some(Press(..))) {
                                    // press->press: current press is mod
                                    mods_currently_held.push(*pressed);
                                }
                                let mut seq_num = u16::from(OsCode::from(pressed));
                                for modk in mods_currently_held.iter().copied() {
                                    seq_num |= mod_mask_for_keycode(modk);
                                }
                                if seq_num & KEY_OVERLAP_MARKER == KEY_OVERLAP_MARKER
                                    && seq_num & MASK_MODDED != KEY_OVERLAP_MARKER
                                {
                                    bail_expr!(
                                        &exprs_remaining[0],
                                        "O-(...) lists cannot be combined with other modifiers."
                                    );
                                }
                                if *pressed != KEY_OVERLAP {
                                    // Note: key overlap item is special and goes at the end,
                                    // not the beginning
                                    seq.push(seq_num);
                                }
                            }
                            Release(released) => {
                                if *released == KEY_OVERLAP {
                                    seq.push(KEY_OVERLAP_MARKER);
                                }
                                if do_release_mod {
                                    mods_currently_held.remove(
                                        mods_currently_held
                                            .iter()
                                            .position(|modk| modk == released)
                                            .expect("had to be pressed to be released"),
                                    );
                                }
                                // release->release: next release is mod
                                do_release_mod = matches!(key_actions.peek(), Some(Release(..)));
                            }
                            _ => unreachable!("should be filtered out"),
                        }
                    }

                    (seq, res.1)
                }
                Err(mut e) => {
                    e.msg = format!("{SEQ_ERR}\nFound invalid key/chord in key_list");
                    return Err(e);
                }
            };
        all_keys.append(&mut keys);
        exprs_remaining = exprs_remaining_tmp;
    }
    Ok(all_keys)
}
