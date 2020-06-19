pub mod tap_dance;
pub mod tap_hold;
pub mod tap_modo;

use crate::effects::KeyCode;
use crate::effects::Effect;
use serde::Deserialize;
pub use tap_dance::TapDanceMgr;
pub use tap_hold::TapHoldMgr;
pub use tap_modo::TapModoMgr;

type TapEffect = Effect;
type HoldEffect = Effect;
type DanceEffect = Effect;
type ModoEffect = Effect;
type Modifier = KeyCode;
type DanceLength = usize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Action {
    /// This is the default keyboard action. Use for simple key remappings.
    Tap(Effect),

    /// Do one `Effect` on a tap different, and a different one on an hold.
    /// E.g CapsLock on a regular tap, and Ctrl while holding.
    TapHold(TapEffect, HoldEffect),

    /// Do one `Effect` on a single tap, and a different one on multiple taps.
    /// E.g 'A' on a regular tap, and CapsLock on 3 quick taps.
    TapDance(DanceLength, TapEffect, DanceEffect),

    /// Do one `Effect` on a tap, and a different one while a modifier is held.
    /// E.g Left/Right arrow keys on a tap, and Home/End while WinKey is held.
    TapModo(TapEffect, Modifier, ModoEffect),

    /// Makes an arbitrary key into a modifier (Ex: for use with TapModo)
    /// In practice, this only drops repeat events.
    Modifier(Modifier)

    /// Escape on regular tap. Tilde when shift is held.
    /// I.E TapModo(Key(KEY_ESC), KEY_LEFTSHIFT, Key(KEY_TILDE))
    TildeEsc,
}
