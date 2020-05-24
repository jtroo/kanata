pub mod tap_hold;
pub mod tap_dance;

pub use tap_hold::TapHoldMgr;
pub use tap_dance::TapDanceMgr;
use crate::effects::Effect;
use serde::Deserialize;

type TapEffect = Effect;
type HoldEffect = Effect;
type DanceEffect = Effect;
type DanceLength = usize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Action {
    Tap(Effect),
    TapHold(TapEffect, HoldEffect),
    TapDance(DanceLength, TapEffect, DanceEffect)

    // Not Implemented Yet
    // -------------------
    // TapDance(DanceLength, Effect, Effect),
    // Sequence(Vec<KeyCode>, Effect),
    // Combo(Vec<KeyCode>, Effect),
}
