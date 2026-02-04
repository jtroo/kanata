//! Information about what state the keyberon layout is in
//! and handling conditional execution based on state.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct ContextualExecution {
    /// Known pause case:
    /// - When replicating output keys during chordv1 activation.
    pub(super) pause_historical_keys_updates: bool,
}

impl ContextualExecution {
    pub(super) fn new() -> Self {
        Self {
            pause_historical_keys_updates: false,
        }
    }

    /// Push into historical keys while checking the pause state.
    pub(super) fn push_historical_key<T: Copy>(&self, h: &mut History<T>, e: T) {
        if !self.pause_historical_keys_updates {
            h.push_front(e);
        }
    }
}
