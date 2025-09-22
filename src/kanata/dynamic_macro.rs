use std::collections::VecDeque;

use kanata_keyberon::layout::Event;
use kanata_parser::cfg::ReplayDelayBehaviour;
use kanata_parser::keys::OsCode;
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicMacroItem {
    Press((OsCode, u16)),
    Release((OsCode, u16)),
    EndMacro(u16),
}

pub struct DynamicMacroReplayState {
    active_macros: HashSet<u16>,
    delay_remaining: u16,
    macro_items: VecDeque<DynamicMacroItem>,
}

pub struct DynamicMacroRecordState {
    starting_macro_id: u16,
    waiting_event: Option<(OsCode, WaitingEventType)>,
    macro_items: Vec<DynamicMacroItem>,
    current_delay: u16,
}

enum WaitingEventType {
    Press,
    Release,
}

impl DynamicMacroRecordState {
    fn new(macro_id: u16) -> Self {
        Self {
            starting_macro_id: macro_id,
            waiting_event: None,
            macro_items: vec![],
            current_delay: 0,
        }
    }

    fn add_release_for_all_unreleased_presses(&mut self) {
        let mut pressed_oscs = HashSet::default();
        for item in self.macro_items.iter() {
            match item {
                DynamicMacroItem::Press((osc, _)) => {
                    pressed_oscs.insert(*osc);
                }
                DynamicMacroItem::Release((osc, _)) => {
                    pressed_oscs.remove(osc);
                }
                DynamicMacroItem::EndMacro(_) => {}
            };
        }
        // Hopefully release order doesn't matter here. A HashSet is being used, meaning release order is arbitrary.
        for osc in pressed_oscs.into_iter() {
            self.macro_items.push(DynamicMacroItem::Release((osc, 0)));
        }
    }

    fn add_event(&mut self, osc: OsCode, evtype: WaitingEventType) {
        if let Some(pending_event) = self.waiting_event.take() {
            match pending_event.1 {
                WaitingEventType::Press => self.macro_items.push(DynamicMacroItem::Press((
                    pending_event.0,
                    self.current_delay,
                ))),
                WaitingEventType::Release => self.macro_items.push(DynamicMacroItem::Release((
                    pending_event.0,
                    self.current_delay,
                ))),
            };
        }
        self.current_delay = 0;
        self.waiting_event = Some((osc, evtype));
    }
}

/// A replay event for a dynamically recorded macro.
/// Note that the key event and the subsequent delay must be processed together.
/// Otherwise there will be real-world time gap between event and the delay,
/// which results in an inaccurate simulation of the keyberon state machine.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReplayEvent(Event, u16);

impl ReplayEvent {
    pub fn key_event(self) -> Event {
        self.0
    }
    pub fn delay(self) -> u16 {
        self.1
    }
}

pub fn tick_record_state(record_state: &mut Option<DynamicMacroRecordState>) {
    if let Some(state) = record_state {
        state.current_delay = state.current_delay.saturating_add(1);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReplayBehaviour {
    pub delay: ReplayDelayBehaviour,
}

pub fn tick_replay_state(
    replay_state: &mut Option<DynamicMacroReplayState>,
    replay_behaviour: ReplayBehaviour,
) -> Option<ReplayEvent> {
    if let Some(state) = replay_state {
        state.delay_remaining = state.delay_remaining.saturating_sub(1);
        if state.delay_remaining == 0 {
            state.delay_remaining = 5;
            match state.macro_items.pop_front() {
                None => {
                    *replay_state = None;
                    log::debug!("finished macro replay");
                    None
                }
                Some(i) => match i {
                    DynamicMacroItem::Press((key, delay)) => {
                        let event = Event::Press(0, key.into());
                        let delay = match replay_behaviour.delay {
                            ReplayDelayBehaviour::Constant => 0,
                            ReplayDelayBehaviour::Recorded => {
                                state.delay_remaining = delay;
                                delay
                            }
                        };
                        Some(ReplayEvent(event, delay))
                    }
                    DynamicMacroItem::Release((key, delay)) => {
                        let event = Event::Release(0, key.into());
                        let delay = match replay_behaviour.delay {
                            ReplayDelayBehaviour::Constant => 0,
                            ReplayDelayBehaviour::Recorded => {
                                state.delay_remaining = delay;
                                delay
                            }
                        };
                        Some(ReplayEvent(event, delay))
                    }
                    DynamicMacroItem::EndMacro(macro_id) => {
                        state.active_macros.remove(&macro_id);
                        None
                    }
                },
            }
        } else {
            None
        }
    } else {
        None
    }
}

pub fn begin_record_macro(
    macro_id: u16,
    record_state: &mut Option<DynamicMacroRecordState>,
) -> Option<(u16, Vec<DynamicMacroItem>)> {
    match record_state.take() {
        None => {
            log::info!("starting dynamic macro {macro_id} recording");
            *record_state = Some(DynamicMacroRecordState::new(macro_id));
            None
        }
        Some(mut state) => {
            if let Some(pending_event) = state.waiting_event.take() {
                match pending_event.1 {
                    WaitingEventType::Press => state.macro_items.push(DynamicMacroItem::Press((
                        pending_event.0,
                        state.current_delay,
                    ))),
                    WaitingEventType::Release => state.macro_items.push(DynamicMacroItem::Release(
                        (pending_event.0, state.current_delay),
                    )),
                };
            }
            // remove the last item, since it's almost certainly a "macro
            // record" key press action which we don't want to keep.
            state.macro_items.remove(state.macro_items.len() - 1);
            state.add_release_for_all_unreleased_presses();

            if state.starting_macro_id == macro_id {
                log::info!(
                    "same macro id pressed. saving and stopping dynamic macro {} recording",
                    state.starting_macro_id
                );
                *record_state = None;
            } else {
                log::info!(
                    "saving dynamic macro {} recording then starting new macro recording {macro_id}",
                    state.starting_macro_id,
                );
                *record_state = Some(DynamicMacroRecordState::new(macro_id));
            }
            Some((state.starting_macro_id, state.macro_items))
        }
    }
}

pub fn record_press(
    record_state: &mut Option<DynamicMacroRecordState>,
    osc: OsCode,
    max_presses: u16,
) -> Option<(u16, Vec<DynamicMacroItem>)> {
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
            Some((state.starting_macro_id, state.macro_items))
        } else {
            log::debug!("delay to press: {}", state.current_delay);
            state.add_event(osc, WaitingEventType::Press);
            None
        }
    } else {
        None
    }
}

pub fn record_release(record_state: &mut Option<DynamicMacroRecordState>, osc: OsCode) {
    if let Some(state) = record_state {
        log::debug!("delay to release: {}", state.current_delay);
        state.add_event(osc, WaitingEventType::Release);
    }
}

pub fn stop_macro(
    record_state: &mut Option<DynamicMacroRecordState>,
    num_actions_to_remove: u16,
) -> Option<(u16, Vec<DynamicMacroItem>)> {
    if let Some(mut state) = record_state.take() {
        if let Some(pending_event) = state.waiting_event.take() {
            match pending_event.1 {
                WaitingEventType::Press => state.macro_items.push(DynamicMacroItem::Press((
                    pending_event.0,
                    state.current_delay,
                ))),
                WaitingEventType::Release => state.macro_items.push(DynamicMacroItem::Release((
                    pending_event.0,
                    state.current_delay,
                ))),
            };
        }
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
        Some((state.starting_macro_id, state.macro_items))
    } else {
        None
    }
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
                log::debug!("playing macro {macro_items:?}");
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
                log::debug!("playing macro {items:?}");
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
