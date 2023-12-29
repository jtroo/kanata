use std::collections::VecDeque;

use kanata_keyberon::layout::Event;
use kanata_parser::keys::OsCode;
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicMacroItem {
    Press(OsCode),
    Release(OsCode),
    EndMacro(u16),
}

pub struct DynamicMacroReplayState {
    pub active_macros: HashSet<u16>,
    pub delay_remaining: u16,
    pub macro_items: VecDeque<DynamicMacroItem>,
}

pub struct DynamicMacroRecordState {
    pub starting_macro_id: u16,
    pub macro_items: Vec<DynamicMacroItem>,
}

impl DynamicMacroRecordState {
    pub fn add_release_for_all_unreleased_presses(&mut self) {
        let mut pressed_oscs = HashSet::default();
        for item in self.macro_items.iter() {
            match item {
                DynamicMacroItem::Press(osc) => pressed_oscs.insert(*osc),
                DynamicMacroItem::Release(osc) => pressed_oscs.remove(osc),
                DynamicMacroItem::EndMacro(_) => false,
            };
        }
        // Hopefully release order doesn't matter here since a HashSet is being used
        for osc in pressed_oscs.into_iter() {
            self.macro_items.push(DynamicMacroItem::Release(osc));
        }
    }
}

pub fn tick_replay_state(state: &mut Option<DynamicMacroReplayState>) -> Option<Event> {
    let mut ret = None;
    let mut clear_replaying_macro = false;
    if let Some(state) = state {
        state.delay_remaining = state.delay_remaining.saturating_sub(1);
        if state.delay_remaining == 0 {
            match state.macro_items.pop_front() {
                None => clear_replaying_macro = true,
                Some(i) => match i {
                    DynamicMacroItem::Press(k) => {
                        ret = Some(Event::Press(0, k.into()));
                    }
                    DynamicMacroItem::Release(k) => {
                        ret = Some(Event::Release(0, k.into()));
                    }
                    DynamicMacroItem::EndMacro(macro_id) => {
                        state.active_macros.remove(&macro_id);
                    }
                },
            }
            state.delay_remaining = 5;
        }
    }
    if clear_replaying_macro {
        log::debug!("finished macro replay");
        *state = None;
    }
    ret
}

pub fn record_macro(
    macro_id: u16,
    record_state: &mut Option<DynamicMacroRecordState>,
) -> Option<(u16, Vec<DynamicMacroItem>)> {
    let mut stop_record = false;
    let mut new_recording = None;
    let mut ret = None;
    match record_state.take() {
        None => {
            log::info!("starting dynamic macro {macro_id} recording");
            *record_state = Some(DynamicMacroRecordState {
                starting_macro_id: macro_id,
                macro_items: vec![],
            })
        }
        Some(mut state) => {
            // remove the last item, since it's almost certainly a "macro
            // record" key press action which we don't want to keep.
            state.macro_items.remove(state.macro_items.len() - 1);
            state.add_release_for_all_unreleased_presses();

            ret = Some((state.starting_macro_id, state.macro_items));
            if state.starting_macro_id == macro_id {
                log::info!(
                    "same macro id pressed. saving and stopping dynamic macro {} recording",
                    state.starting_macro_id
                );
                stop_record = true;
            } else {
                log::info!(
                    "saving dynamic macro {} recording then starting new macro recording {macro_id}",
                    state.starting_macro_id,
                );
                new_recording = Some(macro_id);
            }
        }
    }
    if stop_record {
        *record_state = None;
    } else if let Some(macro_id) = new_recording {
        log::info!("starting new dynamic macro {macro_id} recording");
        *record_state = Some(DynamicMacroRecordState {
            starting_macro_id: macro_id,
            macro_items: vec![],
        });
    }
    ret
}

pub fn stop_macro(
    record_state: &mut Option<DynamicMacroRecordState>,
    num_actions_to_remove: u16,
) -> Option<(u16, Vec<DynamicMacroItem>)> {
    let mut ret = None;
    if let Some(mut state) = record_state.take() {
        // remove the last item independently of `num_actions_to_remove`
        // since it's almost certainly a "macro record stop" key press
        // action which we don't want to keep.
        state.macro_items.remove(state.macro_items.len() - 1);
        log::info!(
            "saving and stopping dynamic macro {} recording with {num_actions_to_remove} actions at the end removed",
            state.starting_macro_id,
        );
        state.macro_items.truncate(
            state
                .macro_items
                .len()
                .saturating_sub(usize::from(num_actions_to_remove)),
        );
        state.add_release_for_all_unreleased_presses();
        ret = Some((state.starting_macro_id, state.macro_items));
    }
    *record_state = None;
    ret
}

pub fn play_macro(
    macro_id: u16,
    replay_state: &mut Option<DynamicMacroReplayState>,
    recorded_macros: &HashMap<u16, Vec<DynamicMacroItem>>,
) {
    match replay_state {
        None => {
            log::info!("replaying macro {macro_id}");
            *replay_state = recorded_macros.get(&macro_id).map(|macro_items| {
                let mut active_macros = HashSet::default();
                active_macros.insert(macro_id);
                DynamicMacroReplayState {
                    active_macros,
                    delay_remaining: 0,
                    macro_items: macro_items.clone().into(),
                }
            });
        }
        Some(state) => {
            if state.active_macros.contains(&macro_id) {
                log::warn!("refusing to recurse into macro {macro_id}");
            } else if let Some(items) = recorded_macros.get(&macro_id) {
                log::debug!("prepending macro {macro_id} items to current replay");
                state.active_macros.insert(macro_id);
                state
                    .macro_items
                    .push_front(DynamicMacroItem::EndMacro(macro_id));
                for item in items.iter().copied().rev() {
                    state.macro_items.push_front(item);
                }
            }
        }
    }
}

pub fn record_press(
    record_state: &mut Option<DynamicMacroRecordState>,
    osc: OsCode,
    max_presses: u16,
) -> Option<(u16, Vec<DynamicMacroItem>)> {
    let mut ret = None;
    if let Some(state) = record_state {
        // This is not 100% accurate since there may be multiple presses before any of
        // their relesease are received. But it's probably good enough in practice.
        //
        // The presses are defined so that a user cares about the number of keys rather
        // than events. So rather than the user multiplying by 2 in their config after
        // considering the number of keys they want, kanata does the multiplication
        // instead.
        if state.macro_items.len() > usize::from(max_presses) * 2 {
            log::warn!(
                "saving and stopping dynamic macro {} recording due to exceeding limit",
                state.starting_macro_id,
            );
            state.add_release_for_all_unreleased_presses();
            let state = record_state.take().unwrap();
            ret = Some((state.starting_macro_id, state.macro_items));
        } else {
            state.macro_items.push(DynamicMacroItem::Press(osc));
        }
    }
    ret
}

pub fn record_release(record_state: &mut Option<DynamicMacroRecordState>, osc: OsCode) {
    if let Some(state) = record_state {
        state.macro_items.push(DynamicMacroItem::Release(osc));
    }
}
