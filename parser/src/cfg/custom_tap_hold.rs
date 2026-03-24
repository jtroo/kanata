use kanata_keyberon::layout::{Event, KCoord, QueuedIter, REAL_KEY_ROW, WaitingAction};

use crate::keys::OsCode;

use super::alloc::Allocations;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Hand {
    Left,
    Right,
    Neutral,
}

/// Compact mapping from key codes to hand assignments.
/// Stores only keys that have an explicit left/right assignment;
/// any key not present is treated as `Hand::Neutral`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HandMap {
    pub(crate) keys: &'static [u16],
    pub(crate) hands: &'static [Hand],
}

impl HandMap {
    pub(crate) fn get(&self, key_code: u16) -> Hand {
        self.keys
            .iter()
            .position(|&k| k == key_code)
            .map(|i| self.hands[i])
            .unwrap_or(Hand::Neutral)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum DecisionBehavior {
    Tap,
    Hold,
    Ignore,
}

/// The function-trait object stored inside `HoldTapConfig::Custom`.
pub(crate) type CustomTapHoldFn =
    dyn Fn(QueuedIter, KCoord) -> (Option<WaitingAction>, bool) + Send + Sync;

/// Returns a closure that can be used in `HoldTapConfig::Custom`, which will return early with a
/// Tap action in the case that any of `keys` are pressed. Otherwise it behaves as
/// `HoldTapConfig::PermissiveHold` would.
pub(crate) fn custom_tap_hold_release(
    keys: &[OsCode],
    a: &Allocations,
) -> &'static CustomTapHoldFn {
    let keys = a.sref_vec(Vec::from_iter(keys.iter().copied()));
    a.sref(
        move |mut queued: QueuedIter, _coord: KCoord| -> (Option<WaitingAction>, bool) {
            while let Some(q) = queued.next() {
                if q.event().is_press() {
                    let (i, j) = q.event().coord();
                    // If any key matches the input, do a tap right away.
                    if i == REAL_KEY_ROW && keys.iter().copied().map(u16::from).any(|j2| j2 == j) {
                        return (Some(WaitingAction::Tap), false);
                    }
                    // Otherwise do the PermissiveHold algorithm.
                    let target = Event::Release(i, j);
                    if queued.clone().copied().any(|q| q.event() == target) {
                        return (Some(WaitingAction::Hold), false);
                    }
                }
            }
            (None, false)
        },
    )
}

/// Returns a closure that can be used in `HoldTapConfig::Custom`, which will return early with a
/// Tap action in the case that any of `keys_press_then_release_trigger_tap` are pressed and
/// released, or if any in `keys_press_trigger_tap` are pressed (no release needed). Otherwise it
/// behaves as `HoldTapConfig::PermissiveHold` would.
pub(crate) fn custom_tap_hold_release_trigger_tap_release(
    keys_press_trigger_tap: &[OsCode],
    keys_press_then_release_trigger_tap: &[OsCode],
    a: &Allocations,
) -> &'static CustomTapHoldFn {
    let keys_press_then_release_trigger_tap = a.sref_vec(Vec::from_iter(
        keys_press_then_release_trigger_tap
            .iter()
            .copied()
            .map(u16::from),
    ));
    let keys_press_trigger_tap = a.sref_vec(Vec::from_iter(
        keys_press_trigger_tap.iter().copied().map(u16::from),
    ));
    a.sref(
        move |mut queued: QueuedIter, _coord: KCoord| -> (Option<WaitingAction>, bool) {
            while let Some(q) = queued.next() {
                if q.event().is_press() {
                    let (i, j) = q.event().coord();
                    if i != REAL_KEY_ROW {
                        continue;
                    }
                    // If any pressed key matches the press list and has been released, do
                    // a tap right away.
                    if keys_press_trigger_tap.iter().copied().any(|j2| j2 == j) {
                        return (Some(WaitingAction::Tap), false);
                    }
                    // If any pressed key matches the press-release list and has been released, do
                    // a tap right away.
                    if keys_press_then_release_trigger_tap
                        .iter()
                        .copied()
                        .any(|j2| j2 == j)
                    {
                        let target = Event::Release(i, j);
                        if queued.clone().copied().any(|q| q.event() == target) {
                            return (Some(WaitingAction::Tap), false);
                        }
                    }
                    // Otherwise do the PermissiveHold algorithm.
                    let target = Event::Release(i, j);
                    if queued.clone().copied().any(|q| q.event() == target) {
                        return (Some(WaitingAction::Hold), false);
                    }
                }
            }
            (None, false)
        },
    )
}

/// Returns a closure for `tap-hold-keys` with three optional key lists:
/// - `keys_tap_on_press`: trigger tap immediately on press
/// - `keys_tap_on_press_release`: trigger tap when pressed then released
/// - `keys_hold_on_press`: trigger hold immediately on press
///
/// For any other key, falls back to PermissiveHold behavior.
///
/// Priority when a key appears in multiple lists (checked in order):
/// tap-on-press > hold-on-press > tap-on-press-release > PermissiveHold
pub(crate) fn custom_tap_hold_keys(
    keys_tap_on_press: &[OsCode],
    keys_tap_on_press_release: &[OsCode],
    keys_hold_on_press: &[OsCode],
    a: &Allocations,
) -> &'static CustomTapHoldFn {
    let keys_tap_on_press = a.sref_vec(keys_tap_on_press.iter().copied().map(u16::from).collect());
    let keys_tap_on_press_release = a.sref_vec(
        keys_tap_on_press_release
            .iter()
            .copied()
            .map(u16::from)
            .collect(),
    );
    let keys_hold_on_press =
        a.sref_vec(keys_hold_on_press.iter().copied().map(u16::from).collect());
    a.sref(
        move |mut queued: QueuedIter, _coord: KCoord| -> (Option<WaitingAction>, bool) {
            while let Some(q) = queued.next() {
                if q.event().is_press() {
                    let (i, j) = q.event().coord();
                    if i != REAL_KEY_ROW {
                        continue;
                    }
                    // If key is in tap-on-press list, trigger tap immediately.
                    if keys_tap_on_press.iter().copied().any(|j2| j2 == j) {
                        return (Some(WaitingAction::Tap), false);
                    }
                    // If key is in hold-on-press list, trigger hold immediately.
                    if keys_hold_on_press.iter().copied().any(|j2| j2 == j) {
                        return (Some(WaitingAction::Hold), false);
                    }
                    // If key is in tap-on-press-release list and has been released,
                    // trigger tap.
                    if keys_tap_on_press_release.iter().copied().any(|j2| j2 == j) {
                        let target = Event::Release(i, j);
                        if queued.clone().copied().any(|q| q.event() == target) {
                            return (Some(WaitingAction::Tap), false);
                        }
                    }
                    // Otherwise do the PermissiveHold algorithm:
                    // if another key was pressed and released, trigger hold.
                    let target = Event::Release(i, j);
                    if queued.clone().copied().any(|q| q.event() == target) {
                        return (Some(WaitingAction::Hold), false);
                    }
                }
            }
            (None, false)
        },
    )
}

pub(crate) fn custom_tap_hold_except(keys: &[OsCode], a: &Allocations) -> &'static CustomTapHoldFn {
    let keys = a.sref_vec(Vec::from_iter(keys.iter().copied()));
    a.sref(
        move |mut queued: QueuedIter, _coord: KCoord| -> (Option<WaitingAction>, bool) {
            for q in queued.by_ref() {
                if q.event().is_press() {
                    let (_i, j) = q.event().coord();
                    // If any key matches the input, do a tap.
                    if keys.iter().copied().map(u16::from).any(|j2| j2 == j) {
                        return (Some(WaitingAction::Tap), false);
                    }
                    // Otherwise continue with default behavior
                    return (None, false);
                }
            }
            // Otherwise skip timeout
            (None, true)
        },
    )
}

/// Returns a closure that can be used in `HoldTapConfig::Custom`, which will return early with a
/// Tap action in the case that any of `keys` are pressed. Unlike `custom_tap_hold_except`, if no
/// matching key is pressed, this waits for timeout instead of skipping it.
pub(crate) fn custom_tap_hold_tap_keys(
    keys: &[OsCode],
    a: &Allocations,
) -> &'static CustomTapHoldFn {
    let keys = a.sref_vec(Vec::from_iter(keys.iter().copied()));
    a.sref(
        move |mut queued: QueuedIter, _coord: KCoord| -> (Option<WaitingAction>, bool) {
            for q in queued.by_ref() {
                if q.event().is_press() {
                    let (_i, j) = q.event().coord();
                    // If any key matches the input, do a tap.
                    if keys.iter().copied().map(u16::from).any(|j2| j2 == j) {
                        return (Some(WaitingAction::Tap), false);
                    }
                    // Otherwise continue with default behavior (no early hold activation)
                }
            }
            // Wait for timeout (key difference from custom_tap_hold_except which returns true)
            (None, false)
        },
    )
}

pub(crate) fn custom_tap_hold_opposite_hand(
    hand_map: &'static HandMap,
    same_hand: DecisionBehavior,
    neutral_behavior: DecisionBehavior,
    unknown_hand: DecisionBehavior,
    neutral_keys: &'static [OsCode],
    a: &Allocations,
) -> &'static CustomTapHoldFn {
    a.sref(
        move |queued: QueuedIter, coord: KCoord| -> (Option<WaitingAction>, bool) {
            let (_row, col) = coord;
            let waiting_hand = hand_map.get(col);

            for q in queued {
                if !q.event().is_press() {
                    continue;
                }
                let (i, j) = q.event().coord();
                if i != REAL_KEY_ROW {
                    continue;
                }

                // Check neutral-keys first (takes precedence over defhands)
                if let Some(osc) = OsCode::from_u16(j) {
                    if neutral_keys.contains(&osc) {
                        match neutral_behavior {
                            DecisionBehavior::Tap => return (Some(WaitingAction::Tap), false),
                            DecisionBehavior::Hold => return (Some(WaitingAction::Hold), false),
                            DecisionBehavior::Ignore => continue,
                        }
                    }
                }

                let pressed_hand = hand_map.get(j);

                match (waiting_hand, pressed_hand) {
                    (Hand::Left, Hand::Right) | (Hand::Right, Hand::Left) => {
                        return (Some(WaitingAction::Hold), false);
                    }
                    (Hand::Left, Hand::Left) | (Hand::Right, Hand::Right) => match same_hand {
                        DecisionBehavior::Tap => return (Some(WaitingAction::Tap), false),
                        DecisionBehavior::Hold => return (Some(WaitingAction::Hold), false),
                        DecisionBehavior::Ignore => continue,
                    },
                    _ => {
                        // At least one key is Neutral (not in defhands)
                        match unknown_hand {
                            DecisionBehavior::Tap => return (Some(WaitingAction::Tap), false),
                            DecisionBehavior::Hold => return (Some(WaitingAction::Hold), false),
                            DecisionBehavior::Ignore => continue,
                        }
                    }
                }
            }
            (None, false)
        },
    )
}

/// Like `custom_tap_hold_opposite_hand` but waits for the interrupting key's
/// press+release before committing. This avoids misfires on fast same-hand
/// rolls where keystrokes briefly overlap.
pub(crate) fn custom_tap_hold_opposite_hand_release(
    hand_map: &'static HandMap,
    same_hand: DecisionBehavior,
    neutral_behavior: DecisionBehavior,
    unknown_hand: DecisionBehavior,
    neutral_keys: &'static [OsCode],
    a: &Allocations,
) -> &'static CustomTapHoldFn {
    a.sref(
        move |mut queued: QueuedIter, coord: KCoord| -> (Option<WaitingAction>, bool) {
            let (_row, col) = coord;
            let waiting_hand = hand_map.get(col);

            while let Some(q) = queued.next() {
                if !q.event().is_press() {
                    continue;
                }
                let (i, j) = q.event().coord();
                if i != REAL_KEY_ROW {
                    continue;
                }

                // Wait for the interrupting key's release before deciding.
                let release = Event::Release(i, j);
                if !queued.clone().copied().any(|q| q.event() == release) {
                    continue;
                }

                // Check neutral-keys first (takes precedence over defhands)
                if let Some(osc) = OsCode::from_u16(j) {
                    if neutral_keys.contains(&osc) {
                        match neutral_behavior {
                            DecisionBehavior::Tap => return (Some(WaitingAction::Tap), false),
                            DecisionBehavior::Hold => return (Some(WaitingAction::Hold), false),
                            DecisionBehavior::Ignore => continue,
                        }
                    }
                }

                let pressed_hand = hand_map.get(j);

                match (waiting_hand, pressed_hand) {
                    (Hand::Left, Hand::Right) | (Hand::Right, Hand::Left) => {
                        return (Some(WaitingAction::Hold), false);
                    }
                    (Hand::Left, Hand::Left) | (Hand::Right, Hand::Right) => match same_hand {
                        DecisionBehavior::Tap => return (Some(WaitingAction::Tap), false),
                        DecisionBehavior::Hold => return (Some(WaitingAction::Hold), false),
                        DecisionBehavior::Ignore => continue,
                    },
                    _ => {
                        // At least one key is Neutral (not in defhands)
                        match unknown_hand {
                            DecisionBehavior::Tap => return (Some(WaitingAction::Tap), false),
                            DecisionBehavior::Hold => return (Some(WaitingAction::Hold), false),
                            DecisionBehavior::Ignore => continue,
                        }
                    }
                }
            }
            (None, false)
        },
    )
}
