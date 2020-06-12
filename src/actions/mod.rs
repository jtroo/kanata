pub mod tap_dance;
pub mod tap_hold;
pub mod tilde_esc;

use crate::effects::Effect;
use serde::Deserialize;
pub use tap_dance::TapDanceMgr;
pub use tap_hold::TapHoldMgr;
pub use tilde_esc::TildeEscMgr;

type TapEffect = Effect;
type HoldEffect = Effect;
type DanceEffect = Effect;
type DanceLength = usize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Action {
    Tap(Effect),
    TapHold(TapEffect, HoldEffect),
    TapDance(DanceLength, TapEffect, DanceEffect),
    TildeEsc,
}
