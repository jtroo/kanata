pub mod tap_dance;
pub mod tap_hold;

use crate::effects::Effect;
use serde::Deserialize;
pub use tap_dance::TapDanceMgr;
pub use tap_hold::TapHoldMgr;

type TapEffect = Effect;
type HoldEffect = Effect;
type DanceEffect = Effect;
type DanceLength = usize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Action {
    Tap(Effect),
    TapHold(TapEffect, HoldEffect),
    TapDance(DanceLength, TapEffect, DanceEffect), // Not Implemented Yet
                                                   // -------------------
                                                   // Sequence(Vec<KeyCode>, Effect),
                                                   // Combo(Vec<KeyCode>, Effect)
}
