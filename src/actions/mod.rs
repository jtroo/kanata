pub mod tap_dance;
pub mod tap_hold;
pub mod tap_mod;

use crate::keys::KeyCode;
use crate::effects::Effect;
use serde::Deserialize;
pub use tap_dance::TapDanceMgr;
pub use tap_hold::TapHoldMgr;
pub use tap_mod::TapModMgr;

type TapEffect = Effect;
type HoldEffect = Effect;
type DanceEffect = Effect;
type ModiEffect = Effect;
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
    /// E.g Left/Right arrow keys on a tap, and Ctrl+N/P while Ctrl is held.
    TapModi(Modifier, TapEffect, ModiEffect),

    /// Same as `TapModi` but also momentarily clears the modifier while performing the effect.
    /// E.g Left/Right arrow keys on a tap, and Home/End while WinKey is held.
    TapModo(Modifier, TapEffect, ModoEffect),

    /// Escape on regular tap. Tilde when shift is held.
    /// I.E TapMod(KEY_LEFTSHIFT, Key(KEY_ESC), Key(KEY_GRAVE))
    TildeEsc,
}
