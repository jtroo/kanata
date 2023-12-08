use kanata_keyberon::layout::{Event, QueuedIter, WaitingAction};

use crate::keys::OsCode;

use super::alloc::Allocations;

/// Returns a closure that can be used in `HoldTapConfig::Custom`, which will return early with a
/// Tap action in the case that any of `keys` are pressed. Otherwise it behaves as
/// `HoldTapConfig::PermissiveHold` would.
pub(crate) fn custom_tap_hold_release(
    keys: &[OsCode],
    a: &Allocations,
) -> &'static (dyn Fn(QueuedIter) -> (Option<WaitingAction>, bool) + Send + Sync) {
    let keys = a.sref_vec(Vec::from_iter(keys.iter().copied()));
    a.sref(
        move |mut queued: QueuedIter| -> (Option<WaitingAction>, bool) {
            while let Some(q) = queued.next() {
                if q.event().is_press() {
                    let (i, j) = q.event().coord();
                    // If any key matches the input, do a tap right away.
                    if keys.iter().copied().map(u16::from).any(|j2| j2 == j) {
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

pub(crate) fn custom_tap_hold_except(
    keys: &[OsCode],
    a: &Allocations,
) -> &'static (dyn Fn(QueuedIter) -> (Option<WaitingAction>, bool) + Send + Sync) {
    let keys = a.sref_vec(Vec::from_iter(keys.iter().copied()));
    a.sref(
        move |mut queued: QueuedIter| -> (Option<WaitingAction>, bool) {
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
