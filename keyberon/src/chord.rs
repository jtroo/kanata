//! Module for chords v2 implementation.

use std::cell::Cell;

use arraydeque::ArrayDeque;
use heapless::Vec as HVec;
use rustc_hash::FxHashMap;

use crate::{
    action::Action,
    key_code::KEY_MAX,
    layout::{Event, Queue, Queued, QueuedAction},
};

// Macro to help with this boilerplate.
// $v should probably be `self` at points of use.
// Ownership rules make this difficult to do as a regular fn,
// because impl function calls don't understand split borrowing.
macro_rules! no_chord_activations {
    ($v:expr) => {{
        $v.ticks_to_ignore_chord = $v.configured_ticks_to_ignore_chord;
    }};
}

pub(crate) const TRIGGER_TAPHOLD_COORD: (u8, u16) = (0, 0);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReleaseBehaviour {
    OnFirstRelease,
    OnLastRelease,
}

#[derive(Debug, Clone)]
pub struct ChordV2<'a, T> {
    /// The action associated with this chord.
    pub action: &'a Action<'a, T>,
    /// The full set of keys that need to be pressed to activate this chord.
    pub participating_keys: &'a [u16],
    /// The number of ticks during which, after the first press of a participant,
    /// this chord can be activated if all participants get pressed.
    /// In other words, after the number of ticks defined by `pending_duration`
    /// elapses, this chord can no longer be completed.
    pub pending_duration: u16,
    /// The layers on which this chord is disabled.
    pub disabled_layers: &'a [u16],
    /// When should the action for this chord be released.
    pub release_behaviour: ReleaseBehaviour,
}

#[derive(Debug, Clone)]
pub struct ChordsForKey<'a, T> {
    /// Chords that this key participates in.
    pub chords: Vec<&'a ChordV2<'a, T>>,
}

#[derive(Debug, Clone)]
pub struct ChordsForKeys<'a, T> {
    pub mapping: FxHashMap<u16, ChordsForKey<'a, T>>,
}

const SMOL_Q_LEN: usize = 16;

struct ActiveChord<'a, T> {
    /// Chords uses a virtual coordinate in the keyberon state for an activated chord.
    /// This field tracks which coordinate to release when the chord itself is released.
    coordinate: u16,
    /// Keys left to release.
    /// For OnFirstRelease, this should have length 0.
    remaining_keys_to_release: HVec<u16, SMOL_Q_LEN>,
    /// Necessary to include here make sure that, for OnFirstRelease,
    /// random other releases that are not part of this chord,
    /// do not release this chord.
    participating_keys: &'a [u16],
    /// Action associated with the active chord.
    /// This needs to be stored here
    action: &'a Action<'a, T>,
    /// In the case of Unread, this chord has not yet been consumed by the layout code.
    /// This might happen for a while because of tap-hold-related delays.
    /// In the Releasable status, the active chord has been consumed and can be released.
    status: ActiveChordStatus,
    /// Tracks how old an action is.
    delay: u16,
}

fn tick_ach<T>(acc: &mut ActiveChord<T>) {
    acc.delay = acc.delay.saturating_add(1);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveChordStatus {
    /// -> UnreadPendingRelease if chord released before being consumed
    /// -> Releasable if consumed
    Unread,
    /// -> Released once consumed
    UnreadReleased,
    /// Can remove at any time.
    /// -> Released once released
    Releasable,
    /// Remove on next tick_chv2
    Released,
}
use ActiveChordStatus::*;

/// Like the layout Queue but smaller.
pub(crate) type SmolQueue = ArrayDeque<Queued, SMOL_Q_LEN, arraydeque::behavior::Wrapping>;

/// Global input chords configuration.
pub struct ChordsV2<'a, T> {
    // Note: Interior fields do not need to be pub or mutable via impl pub fn.
    // Like a layout, this should be destroyed and recreated on a live reload.
    //
    /// Queued inputs that can potentially activate a chord but have not yet.
    /// Inputs will leave if they are determined that they will not activate a chord,
    /// or if a chord activates.
    queue: Queue,
    /// Information about what chords are possible and what keys they are associated with.
    chords: ChordsForKeys<'a, T>,
    /// Chords that are active, i.e. ones that have not yet been released.
    active_chords: HVec<ActiveChord<'a, T>, 10>,
    /// When a key leaves the combo queue without activating a chord,
    /// this activates a timer during which keys cannot activate chords
    /// and are always forwarded directly to the standard input queue.
    ///
    /// This keeps track of the timer.
    ticks_to_ignore_chord: u16,
    /// Initial value for the above when the appropriate event happens.
    /// This must have a minimum value even if not configured by the user,
    /// or if configured by the user to be zero. (maybe forbid that config)
    configured_ticks_to_ignore_chord: u16,
    /// Optimization: if there are no new inputs, the code can skip some processing work.
    /// This tracks the next time that a change will happen, so that the processing work
    /// is **not** skipped when something needs to be checked.
    ticks_until_next_state_change: u16,
    /// Optimization: the below is part of skipping processing work - if this is has changed,
    /// then processing work cannot be skipped.
    prev_active_layer: u16,
    /// Optimization: the below is part of skipping processing work - if this is has changed,
    /// then processing work cannot be skipped.
    prev_queue_len: u8,
    /// Virtual coordinate for use in the layout state.
    next_coord: Cell<u16>,
}

impl<T> std::fmt::Debug for ChordsV2<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChordsV2")
    }
}

impl<'a, T> ChordsV2<'a, T> {
    pub fn new(chords: ChordsForKeys<'a, T>, ticks_ignore_chord: u16) -> Self {
        assert!(ticks_ignore_chord >= 5);
        Self {
            queue: Queue::new(),
            chords,
            active_chords: HVec::new(),
            ticks_to_ignore_chord: 0,
            configured_ticks_to_ignore_chord: ticks_ignore_chord,
            ticks_until_next_state_change: 0,
            prev_active_layer: u16::MAX,
            prev_queue_len: u8::MAX,
            next_coord: Cell::new(KEY_MAX + 1),
        }
    }

    pub fn is_idle_chv2(&self) -> bool {
        self.queue.is_empty() && self.active_chords.is_empty()
    }

    pub fn accepts_chords_chv2(&self) -> bool {
        self.ticks_to_ignore_chord == 0
    }

    pub fn push_back_chv2(&mut self, item: Queued) -> Option<Queued> {
        self.queue.push_back(item)
    }

    pub fn chords(&self) -> &ChordsForKeys<'a, T> {
        &self.chords
    }

    pub(crate) fn get_action_chv2(&mut self) -> QueuedAction<'a, T> {
        self.active_chords
            .iter_mut()
            .find_map(|ach| match ach.status {
                Unread => {
                    ach.status = Releasable;
                    Some(Some(((0, ach.coordinate), ach.delay, ach.action)))
                }
                UnreadReleased => {
                    ach.status = Released;
                    Some(Some(((0, ach.coordinate), ach.delay, ach.action)))
                }
                Releasable | Released => None,
            })
            .unwrap_or_default()
    }

    /// Update the times in the queue without activating any chords yet.
    /// Returns queued events that are no longer usable in chords.
    pub(crate) fn tick_chv2(&mut self, active_layer: u16) -> SmolQueue {
        let mut q = SmolQueue::new();
        self.queue.iter_mut().for_each(Queued::tick_qd);
        let prev_active_chord_len = self.active_chords.len();
        self.active_chords.iter_mut().for_each(tick_ach);
        self.drain_inputs(&mut q, active_layer);
        if self.active_chords.len() != prev_active_chord_len {
            // A chord was activated. Forward a no-op press event to potentially trigger
            // HoldOnOtherKeyPress or PermissiveHold.
            // FLAW: this does not associate with the actual input keys and thus cannot correctly
            // trigger the early tap for *-keys variants of kanata tap-hold.
            q.push_back(Queued::new_press(
                TRIGGER_TAPHOLD_COORD.0,
                TRIGGER_TAPHOLD_COORD.1,
            ));
        }
        if self
            .active_chords
            .iter()
            .any(|ach| matches!(ach.status, UnreadReleased | Released))
        {
            // A chord was released. Forward a no-op release event to potentially trigger
            // PermissiveHold.
            // FLAW: see above
            q.push_back(Queued::new_release(
                TRIGGER_TAPHOLD_COORD.0,
                TRIGGER_TAPHOLD_COORD.1,
            ));
        }
        self.clear_released_chords(&mut q);
        self.ticks_to_ignore_chord = self.ticks_to_ignore_chord.saturating_sub(1);
        q
    }

    fn next_coord(&self) -> u16 {
        let ret = self.next_coord.get();
        let mut new = ret + 1;
        if new > KEY_MAX + 50 {
            new = KEY_MAX + 1;
        }
        self.next_coord.set(new);
        ret
    }

    fn drain_inputs(&mut self, drainq: &mut SmolQueue, active_layer: u16) {
        if self.ticks_to_ignore_chord > 0 {
            drainq.extend(self.queue.drain(0..));
            return;
        }
        if self.ticks_until_next_state_change > 0
            && self.prev_active_layer == active_layer
            && usize::from(self.prev_queue_len) == self.queue.len()
        {
            self.ticks_until_next_state_change =
                self.ticks_until_next_state_change.saturating_sub(1);
            return;
        }
        self.ticks_until_next_state_change = 0;
        self.prev_active_layer = active_layer;
        debug_assert!(self.queue.capacity() < 255);
        self.prev_queue_len = self.queue.len() as u8;

        self.drain_virtual_keys(drainq);
        self.drain_releases(drainq);
        self.process_presses(active_layer);
    }

    fn drain_virtual_keys(&mut self, drainq: &mut SmolQueue) {
        self.queue.retain(|qd| {
            match qd.event {
                // Only row 0 is real inputs.
                // Drain other rows (at the time of writing should only be index 1).
                Event::Press(0, _) | Event::Release(0, _) => true,
                _ => {
                    let overflow = drainq.push_back(*qd);
                    assert!(overflow.is_none(), "oops overflowed drain queue");
                    false
                }
            }
        });
    }

    fn drain_releases(&mut self, drainq: &mut SmolQueue) {
        let achs = &mut self.active_chords;
        let mut presses = HVec::<_, SMOL_Q_LEN>::new();
        self.queue.retain(|qd| match qd.event {
            Event::Press(_, j) => {
                let overflow = presses.push(j);
                debug_assert!(overflow.is_ok());
                true
            }
            Event::Release(_, j) => {
                // Release the key from active chords.
                achs.iter_mut().for_each(|ach| {
                    if !ach.participating_keys.contains(&j) {
                        return;
                    }
                    ach.remaining_keys_to_release.retain(|pk| *pk != j);
                    if ach.remaining_keys_to_release.is_empty() {
                        ach.status = match ach.status {
                            Unread | UnreadReleased => UnreadReleased,
                            Releasable | Released => Released,
                        }
                    }
                });
                if presses.is_empty() {
                    drainq.push_back(*qd);
                    false
                } else {
                    true
                }
            }
        })
    }

    fn process_presses(&mut self, active_layer: u16) {
        let mut presses = HVec::<u16, SMOL_Q_LEN>::new();
        let mut relevant_release_found = false;
        for qd in self.queue.iter() {
            match qd.event {
                Event::Press(_, j) => {
                    let overflowed = presses.push(j);
                    debug_assert!(overflowed.is_ok(), "too many presses in queue");
                }
                Event::Release(_, j) => {
                    if presses.contains(&j) {
                        relevant_release_found = true;
                        break;
                    }
                }
            }
        }
        let prev_active_chords_len = self.active_chords.len();
        let Some(starting_press) = presses.first() else {
            return;
        };
        let Some(possible_chords) = self.chords.mapping.get(starting_press) else {
            no_chord_activations!(self);
            return;
        };

        // For subsequent keypresses,
        // all must fit into a single chord for chord state to remain pending
        // instead of activating a chord,
        // and there must also be a longer chord that can still potentially be activated.
        //
        // Prioritization of chord activation:
        // 1. Timed out chord
        // 2. Longer chord
        let mut accumulated_presses = HVec::<u16, SMOL_Q_LEN>::new();
        let mut chord_candidates = HVec::<&ChordV2<'a, T>, SMOL_Q_LEN>::new();
        let mut timed_out_chord = Option::<(&ChordV2<'a, T>, u8)>::default();
        let mut prev_count = usize::MAX;
        let mut min_timeout;

        assert!(!presses.is_empty());
        let since = self.queue.iter().next().unwrap().since;

        for press in presses.iter().copied() {
            min_timeout = u16::MAX;
            accumulated_presses
                .push(press)
                .expect("accpresses same len as presses");

            let count_possible = if prev_count == chord_candidates.len() {
                // optimization: no longer need to check the whole list.
                // chord_candidates will keep getting shrunk.
                chord_candidates.retain(|chc| chc.participating_keys.contains(&press));
                for chc in chord_candidates.iter() {
                    min_timeout = std::cmp::min(min_timeout, chc.pending_duration);
                }
                chord_candidates.len()
            } else {
                chord_candidates.clear();
                possible_chords
                    .chords
                    .iter()
                    .filter(|pch| !pch.disabled_layers.contains(&active_layer))
                    .filter(|pch| {
                        if accumulated_presses
                            .iter()
                            .all(|acp| pch.participating_keys.contains(acp))
                        {
                            if pch.pending_duration <= since
                                && pch
                                    .participating_keys
                                    .iter()
                                    .all(|pk| accumulated_presses.contains(pk))
                            {
                                // this should only happen at most once per iteration due to needing an exact match.
                                timed_out_chord = Some((pch, accumulated_presses.len() as u8));
                            }
                            // If full, can't run the optimization above, but not fatal.
                            // Can ignore the overflow.
                            let _overflow = chord_candidates.push(pch);
                            min_timeout = std::cmp::min(min_timeout, pch.pending_duration);
                            true
                        } else {
                            false
                        }
                    })
                    .count()
            };

            match count_possible {
                1 => {
                    // Found a chord that is not fully overlapped by another.
                    // Activate the chord if it is completed
                    let coord = self.next_coord();
                    let cch = chord_candidates[0];
                    if cch
                        .participating_keys
                        .iter()
                        .all(|pk| accumulated_presses.contains(pk))
                    {
                        let ach = get_active_chord(cch, since, coord, relevant_release_found);
                        let overflow = self.active_chords.push(ach);
                        assert!(overflow.is_ok(), "active chords has room");
                        break;
                    }
                }
                0 => {
                    // If reached this, it means we went from 2+ -> 0,
                    // or we got to zero at the first iteration.
                    // Backtrack one accumulated press then:
                    // - activate a chord if one completed
                    // - clear the input queue otherwise
                    let _ = accumulated_presses.pop();
                    chord_candidates.clear();
                    let completed_chord = possible_chords
                        .chords
                        .iter()
                        .filter(|pch| !pch.disabled_layers.contains(&active_layer))
                        .find(
                            // Ensure the two lists have the same set of keys
                            |pch| {
                                accumulated_presses
                                    .iter()
                                    .all(|acp| pch.participating_keys.contains(acp))
                                    && pch
                                        .participating_keys
                                        .iter()
                                        .all(|pk| accumulated_presses.contains(pk))
                            },
                        );
                    match completed_chord {
                        Some(cch) => {
                            let coord = self.next_coord();
                            let ach = get_active_chord(cch, since, coord, relevant_release_found);
                            let overflow = self.active_chords.push(ach);
                            assert!(overflow.is_ok(), "active chords has room");
                        }
                        None => no_chord_activations!(self),
                    }
                    break;
                }
                _ => {}
            }
            self.ticks_until_next_state_change = min_timeout.saturating_sub(since);
            prev_count = count_possible;
        }
        if self.ticks_until_next_state_change == 0 || relevant_release_found {
            // Find a chord that matches exactly and activate that,
            // otherwise clear the input queue.
            let completed_chord = if chord_candidates.is_full() {
                possible_chords
                    .chords
                    .iter()
                    .filter(|pch| !pch.disabled_layers.contains(&active_layer))
                    .find(
                        // Ensure the two lists have the same set of keys
                        |pch| {
                            accumulated_presses
                                .iter()
                                .all(|acp| pch.participating_keys.contains(acp))
                                && pch
                                    .participating_keys
                                    .iter()
                                    .all(|pk| accumulated_presses.contains(pk))
                        },
                    )
            } else {
                chord_candidates
                    .iter()
                    .filter(|pch| !pch.disabled_layers.contains(&active_layer))
                    .find(
                        // Ensure the two lists have the same set of keys
                        |pch| {
                            accumulated_presses
                                .iter()
                                .all(|acp| pch.participating_keys.contains(acp))
                                && pch
                                    .participating_keys
                                    .iter()
                                    .all(|pk| accumulated_presses.contains(pk))
                        },
                    )
            };
            match completed_chord {
                Some(cch) => {
                    let ach =
                        get_active_chord(cch, since, self.next_coord(), relevant_release_found);
                    let overflow = self.active_chords.push(ach);
                    assert!(overflow.is_ok(), "active chords has room");
                }
                None => {
                    no_chord_activations!(self)
                }
            }
        }

        // Clear presses from the queue if they were consumed by a chord.
        if self.active_chords.len() > prev_active_chords_len {
            self.queue.retain(|qd| match qd.event {
                Event::Press(_, j) => !accumulated_presses.contains(&j),
                _ => true,
            });
        }
    }

    fn clear_released_chords(&mut self, drainq: &mut SmolQueue) {
        self.active_chords.retain(|ach| {
            if ach.status == Released {
                let overflow = drainq.push_back(Queued {
                    event: Event::Release(0, ach.coordinate),
                    since: 0,
                });
                assert!(overflow.is_none(), "oops overflowed drain queue");
                false
            } else {
                true
            }
        });
    }
}

fn get_active_chord<'a, T>(
    cch: &ChordV2<'a, T>,
    since: u16,
    coord: u16,
    release_found: bool,
) -> ActiveChord<'a, T> {
    let mut remaining_keys_to_release = HVec::new();
    if cch.release_behaviour == ReleaseBehaviour::OnLastRelease {
        remaining_keys_to_release.extend(cch.participating_keys.iter().copied());
    };
    ActiveChord {
        coordinate: coord,
        remaining_keys_to_release,
        participating_keys: cch.participating_keys,
        action: cch.action,
        status: if release_found && cch.release_behaviour == ReleaseBehaviour::OnFirstRelease {
            ActiveChordStatus::UnreadReleased
        } else {
            ActiveChordStatus::Unread
        },
        delay: since,
    }
}
