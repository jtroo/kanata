pub mod tap_hold;
pub use tap_hold::TapHoldMgr;
use crate::effects::Effect;
use serde::Deserialize;

// type DanceCount = usize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Action {
    Tap(Effect),
    TapHold(Effect, Effect),

    // Not Implemented Yet
    // -------------------
    // TapDance(DanceCount, Effect, Effect),
    // Sequence(Vec<KeyCode>, Effect),
    // Combo(Vec<KeyCode>, Effect),
}
