use super::*;

use crate::anyhow_expr;
use crate::bail;
use crate::bail_expr;

const MACRO_ERR: &str = "Action macro only accepts delays, keys, chords, chorded sub-macros, and a subset of special actions.\nThe macro section of the documentation describes this in more detail:\nhttps://github.com/jtroo/kanata/blob/main/docs/config.adoc#macro";

pub(crate) enum RepeatMacro {
    Yes,
    No,
}

pub(crate) fn parse_macro(
    ac_params: &[SExpr],
    s: &ParserState,
    repeat: RepeatMacro,
) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("macro expects at least one item after it")
    }
    let mut all_events = Vec::with_capacity(256);
    let mut params_remainder = ac_params;
    while !params_remainder.is_empty() {
        let mut events;
        (events, params_remainder) = parse_macro_item(params_remainder, s)?;
        all_events.append(&mut events);
    }
    if all_events.iter().any(|e| match e {
        SequenceEvent::Tap(kc) | SequenceEvent::Press(kc) | SequenceEvent::Release(kc) => {
            *kc == KEY_OVERLAP
        }
        _ => false,
    }) {
        bail!("macro contains O- which is only valid within defseq")
    }
    all_events.push(SequenceEvent::Complete);
    all_events.shrink_to_fit();
    match repeat {
        RepeatMacro::No => Ok(s.a.sref(Action::Sequence {
            events: s.a.sref(s.a.sref(s.a.sref_vec(all_events))),
        })),
        RepeatMacro::Yes => Ok(s.a.sref(Action::RepeatableSequence {
            events: s.a.sref(s.a.sref(s.a.sref_vec(all_events))),
        })),
    }
}

pub(crate) fn parse_macro_release_cancel(
    ac_params: &[SExpr],
    s: &ParserState,
    repeat: RepeatMacro,
) -> Result<&'static KanataAction> {
    let macro_action = parse_macro(ac_params, s, repeat)?;
    Ok(s.a.sref(Action::MultipleActions(s.a.sref(s.a.sref_vec(vec![
        *macro_action,
        Action::Custom(s.a.sref(CustomAction::CancelMacroOnRelease)),
    ])))))
}

pub(crate) fn parse_macro_cancel_on_next_press(
    ac_params: &[SExpr],
    s: &ParserState,
    repeat: RepeatMacro,
) -> Result<&'static KanataAction> {
    let macro_action = parse_macro(ac_params, s, repeat)?;
    let macro_duration = match macro_action {
        Action::RepeatableSequence { events } | Action::Sequence { events } => {
            macro_sequence_event_total_duration(events)
        }
        _ => unreachable!("parse_macro should return sequence action"),
    };
    Ok(s.a.sref(Action::MultipleActions(s.a.sref(s.a.sref_vec(vec![
        *macro_action,
        Action::Custom(s.a.sref(CustomAction::CancelMacroOnNextPress(macro_duration))),
    ])))))
}

pub(crate) fn parse_macro_cancel_on_next_press_cancel_on_release(
    ac_params: &[SExpr],
    s: &ParserState,
    repeat: RepeatMacro,
) -> Result<&'static KanataAction> {
    let macro_action = parse_macro(ac_params, s, repeat)?;
    let macro_duration = match macro_action {
        Action::RepeatableSequence { events } | Action::Sequence { events } => {
            macro_sequence_event_total_duration(events)
        }
        _ => unreachable!("parse_macro should return sequence action"),
    };
    Ok(s.a.sref(Action::MultipleActions(s.a.sref(s.a.sref_vec(vec![
        *macro_action,
        Action::Custom(s.a.sref(CustomAction::CancelMacroOnRelease)),
        Action::Custom(s.a.sref(CustomAction::CancelMacroOnNextPress(macro_duration))),
    ])))))
}

pub(crate) fn macro_sequence_event_total_duration<T>(events: &[SequenceEvent<T>]) -> u32 {
    events.iter().fold(0, |duration, event| {
        duration.saturating_add(match event {
            SequenceEvent::Delay { duration: d } => *d,
            _ => 1,
        })
    })
}

pub(crate) fn parse_macro_record_stop_truncate(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_STR: &str =
        "dynamic-macro-record-stop-truncate expects 1 param: <num-keys-to-truncate>";
    if ac_params.len() != 1 {
        bail!("{ERR_STR}\nFound {} params instead of 1", ac_params.len());
    }
    let num_to_truncate = parse_u16(&ac_params[0], s, "num-keys-to-truncate")?;
    custom(CustomAction::DynamicMacroRecordStop(num_to_truncate), &s.a)
}

#[derive(PartialEq)]
pub(crate) enum MacroNumberParseMode {
    Delay,
    Action,
}

#[allow(clippy::type_complexity)] // return type is not pub
pub(crate) fn parse_macro_item<'a>(
    acs: &'a [SExpr],
    s: &ParserState,
) -> Result<(Vec<SequenceEvent<'static, KanataCustom>>, &'a [SExpr])> {
    parse_macro_item_impl(acs, s, MacroNumberParseMode::Delay)
}

#[allow(clippy::type_complexity)] // return type is not pub
pub(crate) fn parse_macro_item_impl<'a>(
    acs: &'a [SExpr],
    s: &ParserState,
    num_parse_mode: MacroNumberParseMode,
) -> Result<(Vec<SequenceEvent<'static, KanataCustom>>, &'a [SExpr])> {
    if num_parse_mode == MacroNumberParseMode::Delay {
        if let Some(a) = acs[0].atom(s.vars()) {
            match parse_non_zero_u16(&acs[0], s, "delay") {
                Ok(duration) => {
                    let duration = u32::from(duration);
                    return Ok((vec![SequenceEvent::Delay { duration }], &acs[1..]));
                }
                Err(e) => {
                    if a.chars().all(|c| c.is_ascii_digit()) {
                        return Err(e);
                    }
                }
            }
        }
    }
    match parse_action(&acs[0], s) {
        Ok(Action::KeyCode(kc)) => {
            // Should note that I tried `SequenceEvent::Tap` initially but it seems to be buggy
            // so I changed the code to use individual press and release. The SequenceEvent
            // code is from a PR that (at the time of this writing) hasn't yet been merged into
            // keyberon master and doesn't have tests written for it yet. This seems to work as
            // expected right now though.
            Ok((
                vec![SequenceEvent::Press(*kc), SequenceEvent::Release(*kc)],
                &acs[1..],
            ))
        }
        Ok(Action::MultipleKeyCodes(kcs)) => {
            // chord - press in order then release in the reverse order
            let mut events = vec![];
            for kc in kcs.iter() {
                events.push(SequenceEvent::Press(*kc));
            }
            for kc in kcs.iter().rev() {
                events.push(SequenceEvent::Release(*kc));
            }
            Ok((events, &acs[1..]))
        }
        Ok(Action::Custom(custom)) => Ok((vec![SequenceEvent::Custom(custom)], &acs[1..])),
        Ok(_) => bail_expr!(&acs[0], "{MACRO_ERR}"),
        Err(e) => {
            if let Some(submacro) = acs[0].list(s.vars()) {
                // If it's just a list that's not parsable as a usable action, try parsing the
                // content.
                let mut submacro_remainder = submacro;
                let mut all_events = vec![];
                while !submacro_remainder.is_empty() {
                    let mut events;
                    (events, submacro_remainder) =
                        parse_macro_item(submacro_remainder, s).map_err(|_e| e.clone())?;
                    all_events.append(&mut events);
                }
                return Ok((all_events, &acs[1..]));
            }

            let (held_mods, unparsed_str) =
                parse_mods_held_for_submacro(&acs[0], s).map_err(|mut err| {
                    if err.msg == MACRO_ERR {
                        err.msg = format!("{}\n{MACRO_ERR}", &e.msg);
                    }
                    err
                })?;
            let mut all_events = vec![];

            // First, press all of the modifiers
            for kc in held_mods.iter().copied() {
                all_events.push(SequenceEvent::Press(kc));
            }

            let mut rem_start = 1;
            let maybe_list_var = SExpr::Atom(Spanned::new(unparsed_str.into(), acs[0].span()));
            let submacro = match maybe_list_var.list(s.vars()) {
                Some(l) => l,
                None => {
                    // Ensure that the unparsed text is empty since otherwise it means there is
                    // invalid text there
                    if !unparsed_str.is_empty() {
                        bail_expr!(&acs[0], "{}\n{MACRO_ERR}", &e.msg)
                    }
                    // Check for a follow-up list
                    rem_start = 2;
                    if acs.len() < 2 {
                        bail_expr!(&acs[0], "{}\n{MACRO_ERR}", &e.msg)
                    }
                    acs[1]
                        .list(s.vars())
                        .ok_or_else(|| anyhow_expr!(&acs[1], "{MACRO_ERR}"))?
                }
            };
            let mut submacro_remainder = submacro;
            let mut events;
            while !submacro_remainder.is_empty() {
                (events, submacro_remainder) = parse_macro_item(submacro_remainder, s)?;
                all_events.append(&mut events);
            }

            // Lastly, release modifiers
            for kc in held_mods.iter().copied() {
                all_events.push(SequenceEvent::Release(kc));
            }

            Ok((all_events, &acs[rem_start..]))
        }
    }
}

/// Parses mod keys like `C-S-`. Returns the `KeyCode`s for the modifiers parsed and the unparsed
/// text after any parsed modifier prefixes.
pub(crate) fn parse_mods_held_for_submacro<'a>(
    held_mods: &'a SExpr,
    s: &'a ParserState,
) -> Result<(Vec<KeyCode>, &'a str)> {
    let mods = held_mods
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(held_mods, "{MACRO_ERR}"))?;
    let (mod_keys, unparsed_str) = parse_mod_prefix(mods)?;
    if mod_keys.is_empty() {
        bail_expr!(held_mods, "{MACRO_ERR}");
    }
    Ok((mod_keys, unparsed_str))
}

pub(crate) fn parse_dynamic_macro_record(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "dynamic-macro-record expects 1 parameter: <macro ID (0-65535)>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let key = parse_u16(&ac_params[0], s, "macro ID")?;
    custom(CustomAction::DynamicMacroRecord(key), &s.a)
}

pub(crate) fn parse_dynamic_macro_play(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "dynamic-macro-play expects 1 parameter: <macro ID (number 0-65535)>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let key = parse_u16(&ac_params[0], s, "macro ID")?;
    custom(CustomAction::DynamicMacroPlay(key), &s.a)
}
