//! Game controller support: the control vocabulary and the configuration tree
//! `defgamepad` produces.
//!
//! ```text
//! backend (gilrs)
//!   -> PadInput   one whole reading, already normalized
//!   -> Projector  deadzones, thresholds, SOCD
//!   -> PadSet     every control held right now, in one u64
//!   -> PadEdge    the diff against last time  -> the keyberon graph
//!   -> Demand     a rate  -> kanata's existing mouse primitives, per tick
//! ```
//!
//! Analog values never become keys: an `OsCode` is a binary coordinate in a
//! layout row, so only threshold crossings and digital controls enter the
//! layout and the value itself stays in `f32` to the point of use. And
//! `defgamepad` only declares controls -- what a projected control *does* is
//! `defsrc`, `deflayer` and every action kanata already has, which is why
//! `pad-south` composes with `tap-hold`, layers and macros for free.
//!
//! Everything from `PadInput` onwards is arithmetic with no platform type
//! anywhere, so it can be tested from recorded traces rather than from a
//! controller on someone's desk. The parser half lives here because configs
//! are parsed in builds with no backend; the runtime half is
//! `kanata::gamepad`.

pub mod analog;
pub mod project;

use core::fmt;
use core::ops::{BitOr, BitOrAssign};
use std::num::NonZeroU8;

use crate::keys::OsCode;

pub use analog::{AxisValue, Curve, Demand, Motion, MotionKind, StickPosition, Unit, Vec2};
pub use project::{PadInput, Projector};

// ---------------------------------------------------------------- vocabulary

/// Which of a paired control -- a stick, a trigger -- is meant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Left, Side::Right];

    pub const fn as_str(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }

    /// The control this side's trigger presses once past its band.
    ///
    /// Most hardware reports a trigger twice, as a button and as an analog
    /// axis. Both drive this control and the projector holds it while either
    /// says so, so a pad reporting one still works and a pad reporting both
    /// does not double-fire.
    pub const fn trigger(self) -> PadButton {
        match self {
            Side::Left => PadButton::L2,
            Side::Right => PadButton::R2,
        }
    }

    pub const fn stick(self) -> Directional {
        match self {
            Side::Left => Directional::LeftStick,
            Side::Right => Directional::RightStick,
        }
    }
}

/// A control that points a direction: the two sticks and the d-pad.
///
/// One type for all three because they project identically -- thresholds,
/// SOCD, four- or eight-way, and an optional continuous output. A d-pad is a
/// stick that only knows nine positions, and treating it as one is what gives
/// it eight-way output and pointer control without a second code path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Directional {
    LeftStick,
    RightStick,
    Dpad,
}

impl Directional {
    pub const ALL: [Directional; 3] = [
        Directional::LeftStick,
        Directional::RightStick,
        Directional::Dpad,
    ];

    /// The side this is a stick of, or `None` for the d-pad. Also the index
    /// into the per-stick state a d-pad does not have.
    pub const fn side(self) -> Option<Side> {
        match self {
            Directional::LeftStick => Some(Side::Left),
            Directional::RightStick => Some(Side::Right),
            Directional::Dpad => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Directional::LeftStick => "left stick",
            Directional::RightStick => "right stick",
            Directional::Dpad => "d-pad",
        }
    }
}
