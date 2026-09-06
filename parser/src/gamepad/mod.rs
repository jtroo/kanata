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

/// One of eight directions a projected control can expose to the layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl Dir {
    pub const ALL: [Dir; 8] = [
        Dir::Up,
        Dir::Down,
        Dir::Left,
        Dir::Right,
        Dir::UpLeft,
        Dir::UpRight,
        Dir::DownLeft,
        Dir::DownRight,
    ];

    pub const fn is_diagonal(self) -> bool {
        (self as u8) >= Dir::UpLeft as u8
    }

    /// The direction that cancels this one, which is what SOCD arbitrates.
    pub const fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
            Dir::UpLeft => Dir::DownRight,
            Dir::UpRight => Dir::DownLeft,
            Dir::DownLeft => Dir::UpRight,
            Dir::DownRight => Dir::UpLeft,
        }
    }

    /// The unit vector this direction points along, y-up.
    pub const fn vector(self) -> Vec2 {
        let (x, y) = match self {
            Dir::Up => (0.0, 1.0),
            Dir::Down => (0.0, -1.0),
            Dir::Left => (-1.0, 0.0),
            Dir::Right => (1.0, 0.0),
            Dir::UpLeft => (-1.0, 1.0),
            Dir::UpRight => (1.0, 1.0),
            Dir::DownLeft => (-1.0, -1.0),
            Dir::DownRight => (1.0, -1.0),
        };
        Vec2 { x, y }
    }
}

/// One of the four directions a physical axis or contact can assert.
///
/// This is deliberately distinct from [`Dir`]. Hardware reports cardinals;
/// diagonals are a projection of one vertical and one horizontal cardinal.
/// Keeping those domains separate prevents an already-projected diagonal from
/// being fed back into SOCD or four-way projection as if it were raw input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cardinal {
    Up,
    Down,
    Left,
    Right,
}

impl Cardinal {
    pub const ALL: [Cardinal; 4] = [
        Cardinal::Up,
        Cardinal::Down,
        Cardinal::Left,
        Cardinal::Right,
    ];

    pub const fn direction(self) -> Dir {
        match self {
            Cardinal::Up => Dir::Up,
            Cardinal::Down => Dir::Down,
            Cardinal::Left => Dir::Left,
            Cardinal::Right => Dir::Right,
        }
    }

    pub const fn opposite(self) -> Cardinal {
        match self {
            Cardinal::Up => Cardinal::Down,
            Cardinal::Down => Cardinal::Up,
            Cardinal::Left => Cardinal::Right,
            Cardinal::Right => Cardinal::Left,
        }
    }

    pub const fn vector(self) -> Vec2 {
        self.direction().vector()
    }
}

impl TryFrom<Dir> for Cardinal {
    type Error = ();

    fn try_from(dir: Dir) -> Result<Cardinal, ()> {
        match dir {
            Dir::Up => Ok(Cardinal::Up),
            Dir::Down => Ok(Cardinal::Down),
            Dir::Left => Ok(Cardinal::Left),
            Dir::Right => Ok(Cardinal::Right),
            Dir::UpLeft | Dir::UpRight | Dir::DownLeft | Dir::DownRight => Err(()),
        }
    }
}

/// A set of raw cardinal assertions from a stick, d-pad, or hat.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct CardinalSet(u8);

impl CardinalSet {
    pub const EMPTY: CardinalSet = CardinalSet(0);

    pub const fn of(dirs: &[Cardinal]) -> CardinalSet {
        let (mut bits, mut i) = (0u8, 0);
        while i < dirs.len() {
            bits |= 1 << dirs[i] as u8;
            i += 1;
        }
        CardinalSet(bits)
    }

    pub const fn contains(self, dir: Cardinal) -> bool {
        self.0 & (1 << dir as u8) != 0
    }

    pub fn set(&mut self, dir: Cardinal, present: bool) {
        let bit = 1 << dir as u8;
        self.0 = if present { self.0 | bit } else { self.0 & !bit };
    }

    pub fn iter(self) -> impl Iterator<Item = Cardinal> {
        Cardinal::ALL.into_iter().filter(move |d| self.contains(*d))
    }

    /// The direction this set asserts on one axis, or `None` when it asserts
    /// neither or both. Both is not a direction: it is a question for SOCD,
    /// and nothing here may guess an answer.
    const fn axis(self, positive: Cardinal, negative: Cardinal) -> Option<Cardinal> {
        match (self.contains(positive), self.contains(negative)) {
            (true, false) => Some(positive),
            (false, true) => Some(negative),
            _ => None,
        }
    }

    /// The single direction a set of cardinals denotes, or `None` if it
    /// denotes none.
    ///
    /// The two axes are resolved separately. An axis asserting both directions
    /// contributes nothing, but it must not take the other axis down with it:
    /// a hitbox holding Left+Right while pushing Up is still pushing Up, and
    /// collapsing that to "no direction at all" would release a control the
    /// user never let go of.
    pub const fn single(self) -> Option<Dir> {
        match (
            self.axis(Cardinal::Up, Cardinal::Down),
            self.axis(Cardinal::Right, Cardinal::Left),
        ) {
            (Some(vertical), Some(horizontal)) => Some(combine(vertical, horizontal)),
            (Some(found), None) | (None, Some(found)) => Some(found.direction()),
            (None, None) => None,
        }
    }

    /// The directions that reach the layout in the requested mode.
    pub fn projected(self, mode: DirMode) -> impl Iterator<Item = Dir> {
        let single = self.single();
        Dir::ALL.into_iter().filter(move |dir| match mode {
            DirMode::FourWay => Cardinal::try_from(*dir).is_ok_and(|d| self.contains(d)),
            DirMode::EightWay => single == Some(*dir),
        })
    }

    /// Where this set points, with components in `-1..=1`. A d-pad reads as a
    /// stick this way, so both drive the same continuous projection.
    pub fn vector(self) -> Vec2 {
        self.iter()
            .map(Cardinal::vector)
            .fold(Vec2::ZERO, |total, v| total + v)
    }
}

/// The diagonal two cardinals make. Only reachable with one vertical and one
/// horizontal, which is the only way [`CardinalSet::single`] calls it.
const fn combine(a: Cardinal, b: Cardinal) -> Dir {
    match (a, b) {
        (Cardinal::Up, Cardinal::Left) | (Cardinal::Left, Cardinal::Up) => Dir::UpLeft,
        (Cardinal::Up, Cardinal::Right) | (Cardinal::Right, Cardinal::Up) => Dir::UpRight,
        (Cardinal::Down, Cardinal::Left) | (Cardinal::Left, Cardinal::Down) => Dir::DownLeft,
        (Cardinal::Down, Cardinal::Right) | (Cardinal::Right, Cardinal::Down) => Dir::DownRight,
        _ => unreachable!(),
    }
}

impl BitOr for CardinalSet {
    type Output = CardinalSet;
    fn bitor(self, other: CardinalSet) -> CardinalSet {
        CardinalSet(self.0 | other.0)
    }
}

impl fmt::Debug for CardinalSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

/// A digital control with a portable meaning.
///
/// Every name is positional: [`PadButton::South`] is the lower face button
/// whatever glyph the vendor printed on it, so one configuration works across
/// a DualSense, an Xbox pad and a Switch Pro pad without edits. The d-pad is
/// not here: its four contacts point a direction, so it is a [`Directional`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    /// Lower face button: Cross, A, or B depending on the vendor.
    South,
    /// Right face button: Circle, B, or A.
    East,
    /// Left face button: Square, X, or Y.
    West,
    /// Upper face button: Triangle, Y, or X.
    North,
    /// Extra face buttons present on some six-button pads.
    C,
    Z,
    /// Upper shoulder buttons.
    L1,
    R1,
    /// Triggers, as digital controls. See [`Side::trigger`].
    L2,
    R2,
    Select,
    Start,
    /// The vendor button: PS, Guide, Home.
    Mode,
    /// Stick clicks.
    L3,
    R3,
}

impl PadButton {
    pub const ALL: [PadButton; 15] = [
        PadButton::South,
        PadButton::East,
        PadButton::West,
        PadButton::North,
        PadButton::C,
        PadButton::Z,
        PadButton::L1,
        PadButton::R1,
        PadButton::L2,
        PadButton::R2,
        PadButton::Select,
        PadButton::Start,
        PadButton::Mode,
        PadButton::L3,
        PadButton::R3,
    ];
}

