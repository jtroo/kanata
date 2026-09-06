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

// -------------------------------------------------------------------- codes

/// A control on a game controller, as an index into the `OsCode` range
/// reserved for them.
///
/// The only supported way to obtain a controller `OsCode`: keeping the
/// synthetic range behind a constructor stops it leaking into code that
/// reasons about real scancodes. The range is laid out as buttons, then eight
/// directions per [`Directional`], then the slots, so construction and
/// [`PadCode::control`] are index arithmetic rather than a lookup table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PadCode(u8);

impl PadCode {
    /// How many `pad-button-N` slots exist.
    pub const SLOTS: u8 = 16;

    const DIRECTIONS: u8 = PadButton::ALL.len() as u8;
    const SLOT_BASE: u8 = PadCode::DIRECTIONS + (Directional::ALL.len() * Dir::ALL.len()) as u8;

    pub const fn button(button: PadButton) -> PadCode {
        PadCode(button as u8)
    }

    pub const fn direction(control: Directional, dir: Dir) -> PadCode {
        PadCode(PadCode::DIRECTIONS + control as u8 * Dir::ALL.len() as u8 + dir as u8)
    }

    pub const fn slot(index: u8) -> Option<PadCode> {
        match index < PadCode::SLOTS {
            true => Some(PadCode(PadCode::SLOT_BASE + index)),
            false => None,
        }
    }

    /// Recover a `PadCode`, rejecting anything outside the reserved range.
    pub const fn from_os_code(code: OsCode) -> Option<PadCode> {
        match code.gamepad_index() {
            Some(index) => Some(PadCode(index)),
            None => None,
        }
    }

    pub const fn os_code(self) -> OsCode {
        match OsCode::from_gamepad_index(self.0) {
            Some(code) => code,
            None => unreachable!(),
        }
    }

    /// What this code names: the inverse of the constructors, so callers can
    /// reason about a code without re-deriving the layout of the range.
    pub const fn control(self) -> PadControl {
        let index = self.0;
        if index < PadCode::DIRECTIONS {
            return PadControl::Button(PadButton::ALL[index as usize]);
        }
        if index < PadCode::SLOT_BASE {
            let rest = (index - PadCode::DIRECTIONS) as usize;
            return PadControl::Direction(
                Directional::ALL[rest / Dir::ALL.len()],
                Dir::ALL[rest % Dir::ALL.len()],
            );
        }
        PadControl::Slot(index - PadCode::SLOT_BASE)
    }
}

/// The vocabulary above and the reserved `OsCode` range are two spellings of
/// one thing, and every constructor here assumes they agree.
const _: () = {
    assert!(
        PadCode::SLOT_BASE + PadCode::SLOTS == OsCode::GAMEPAD_COUNT,
        "the reserved OsCode range and the control vocabulary disagree"
    );
    assert!(
        OsCode::GAMEPAD_COUNT as u32 <= u64::BITS,
        "a PadSet has one bit per control; widen it or shrink the vocabulary"
    );
};

/// The three kinds of control the reserved range holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadControl {
    Button(PadButton),
    Direction(Directional, Dir),
    /// A user-assignable `pad-button-N`.
    Slot(u8),
}

impl From<PadCode> for OsCode {
    fn from(code: PadCode) -> OsCode {
        code.os_code()
    }
}

impl fmt::Debug for PadCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.os_code())
    }
}

impl fmt::Display for PadCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.os_code())
    }
}

/// A digital control changing state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadEdge {
    pub code: PadCode,
    pub pressed: bool,
}

/// The set of controls a controller is holding.
///
/// One `u64`, because the reserved range is deliberately smaller than that.
/// Holding state, merging several controllers and diffing for edges are then
/// each a single instruction, and nothing on the event path allocates.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct PadSet(u64);

impl PadSet {
    pub const EMPTY: PadSet = PadSet(0);

    pub const fn contains(self, code: PadCode) -> bool {
        self.0 & (1 << code.0) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn insert(&mut self, code: PadCode) {
        self.0 |= 1 << code.0;
    }

    pub fn set(&mut self, code: PadCode, present: bool) {
        let bit = 1u64 << code.0;
        self.0 = if present { self.0 | bit } else { self.0 & !bit };
    }

    pub fn iter(self) -> impl Iterator<Item = PadCode> {
        codes(self.0)
    }

    /// The transitions from `self` to `next`.
    ///
    /// Releases come before presses: a four-way stick moving from Up to Left
    /// must not momentarily hold both, which a game reads as a diagonal. Every
    /// control group diffs through here, so that rule is written once.
    pub fn edges(self, next: PadSet) -> impl Iterator<Item = PadEdge> {
        let released = codes(self.0 & !next.0).map(|code| PadEdge {
            code,
            pressed: false,
        });
        let pressed = codes(next.0 & !self.0).map(|code| PadEdge {
            code,
            pressed: true,
        });
        released.chain(pressed)
    }
}

fn codes(mut bits: u64) -> impl Iterator<Item = PadCode> {
    core::iter::from_fn(move || {
        (bits != 0).then(|| {
            let index = bits.trailing_zeros() as u8;
            bits &= bits - 1;
            PadCode(index)
        })
    })
}

impl BitOr for PadSet {
    type Output = PadSet;
    fn bitor(self, other: PadSet) -> PadSet {
        PadSet(self.0 | other.0)
    }
}

impl BitOrAssign for PadSet {
    fn bitor_assign(&mut self, other: PadSet) {
        self.0 |= other.0;
    }
}

impl fmt::Debug for PadSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

// ------------------------------------------------------------------- config
//
// A description of physical controls and their projections, and nothing else:
// it never names an output key.

/// How many cardinals a diagonal presses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DirMode {
    /// A diagonal presses two cardinal controls, the way holding two keys
    /// does. Right for WASD-style movement.
    #[default]
    FourWay,
    /// A diagonal presses one dedicated diagonal control instead; a straight
    /// push still presses its cardinal. Right when a diagonal has to be
    /// distinguishable from its components.
    EightWay,
}

/// How to resolve a control asserting both directions of an axis at once.
///
/// Only a control with independent contacts can do that -- a d-pad, or a
/// hitbox-style stick replacement. A stick reports each axis as one signed
/// number, so the setting is inert there.
///
/// The default is to pass both through, because both contacts really are down;
/// the other modes exist for people who want a game's semantics. Resolution is
/// per controller: two pads pressing opposite directions is two players, not a
/// conflict, and the engine unions them afterwards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Socd {
    /// Both directions reach the layout. The default.
    #[default]
    Off,
    /// Neither direction survives. The fighting-game convention.
    Neutral,
    /// The most recently pressed direction wins, and takes over immediately.
    Last,
    /// The direction already held wins; the newcomer is ignored until the
    /// incumbent releases.
    First,
    /// Up and Right always win.
    Positive,
    /// Down and Left always win.
    Negative,
}

/// Threshold crossings become `pad-lstick-*`, `pad-rstick-*` or `pad-dpad-*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Digital {
    pub mode: DirMode,
    pub socd: Socd,
    /// Where each axis presses. Compared against the *raw* reading, so
    /// `(threshold 0.5)` means half of physical deflection and nothing
    /// -- deadzone or curve -- can silently move it. Ignored by a d-pad, which
    /// is digital before it arrives.
    pub threshold: Unit,
}

/// At most one projection for each continuous output domain.
///
/// A typed two-slot map is both less error-prone than an
/// `Option<(MotionKind, T)>` and more capable: a control may intentionally
/// drive the pointer and wheel together, while duplicate declarations of the
/// same kind remain a parser error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motions<T>([Option<T>; MotionKind::ALL.len()]);

impl<T> Default for Motions<T> {
    fn default() -> Motions<T> {
        Motions([None, None])
    }
}

impl<T: Copy> Motions<T> {
    pub fn get(&self, kind: MotionKind) -> Option<T> {
        self.0[kind as usize]
    }

    pub fn set(&mut self, kind: MotionKind, value: T) {
        self.0[kind as usize] = Some(value);
    }

    pub fn contains(self, kind: MotionKind) -> bool {
        self.get(kind).is_some()
    }

    pub fn is_empty(self) -> bool {
        self.0.iter().all(Option::is_none)
    }
}

impl Default for Digital {
    fn default() -> Digital {
        Digital {
            mode: DirMode::default(),
            socd: Socd::default(),
            threshold: Unit::new(0.50),
        }
    }
}

/// What a stick or d-pad does.
///
/// The two halves are independent and a control may have both: a stick can
/// drive the pointer *and* press a key at full deflection, because the
/// threshold and the displacement read the same value without disturbing each
/// other.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Projection {
    pub digital: Option<Digital>,
    pub motions: Motions<Motion>,
}

impl Projection {
    /// What a stick starts with: nothing. Its values are still tracked, so a
    /// live reload can begin using it without waiting for the user to move it.
    pub const OFF: Projection = Projection {
        digital: None,
        motions: Motions([None, None]),
    };
}

/// What a trigger's analog half does.
///
/// Crossing the threshold presses `pad-l2` / `pad-r2`. A trigger is one number
/// rather than a vector, so a continuous projection has to be told which way
/// to push -- which is also what makes `(trigger right (scroll down ...))` a
/// pressure-sensitive scroll wheel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trigger {
    pub threshold: Unit,
    pub motions: Motions<TriggerMotion>,
}

/// A scalar trigger projected along one cardinal direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerMotion {
    pub direction: Cardinal,
    pub motion: Motion,
}

impl Default for Trigger {
    fn default() -> Trigger {
        // A trigger actuates much earlier than a stick.
        Trigger {
            threshold: Unit::new(0.30),
            motions: Motions::default(),
        }
    }
}

/// Everything `defgamepad` declares.
///
/// `Copy` on purpose: it is handed to a projector per connected controller and
/// replaced wholesale on live reload, and neither path should have to think
/// about sharing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadConfig {
    /// Restricts this declaration to one `definputdevices` entry. `None` means
    /// every connected controller feeds the same controls, which is what a
    /// single-pad user wants and a two-pad user overrides.
    pub device: Option<NonZeroU8>,
    /// Indexed by [`Directional`].
    pub directionals: [Projection; Directional::ALL.len()],
    /// Indexed by [`Side`].
    pub triggers: [Trigger; Side::ALL.len()],
    /// The backend code bound to each `pad-button-N`.
    ///
    /// Controllers expose paddles, extra hats and vendor buttons that no
    /// portable name can describe. Rather than invent an unstable
    /// auto-numbering, kanata offers slots and lets the user bind a code to
    /// each; the codes it cannot name are printed to the log as they arrive.
    pub slots: [Option<u32>; PadCode::SLOTS as usize],
}

impl Default for GamepadConfig {
    fn default() -> GamepadConfig {
        let mut directionals = [Projection::OFF; Directional::ALL.len()];
        // A d-pad needs no declaration: it is digital hardware, and there is
        // nothing to decide before it can press a key.
        directionals[Directional::Dpad as usize].digital = Some(Digital::default());
        GamepadConfig {
            device: None,
            directionals,
            triggers: [Trigger::default(); Side::ALL.len()],
            slots: [None; PadCode::SLOTS as usize],
        }
    }
}

impl GamepadConfig {
    pub fn projection(&self, control: Directional) -> &Projection {
        &self.directionals[control as usize]
    }

    pub fn projection_mut(&mut self, control: Directional) -> &mut Projection {
        &mut self.directionals[control as usize]
    }

    pub fn trigger(&self, side: Side) -> &Trigger {
        &self.triggers[side as usize]
    }

    pub fn trigger_mut(&mut self, side: Side) -> &mut Trigger {
        &mut self.triggers[side as usize]
    }

    /// The slot a backend code is bound to, if any.
    pub fn slot_of(&self, code: u32) -> Option<PadCode> {
        let index = self.slots.iter().position(|bound| *bound == Some(code))?;
        PadCode::slot(index as u8)
    }

    /// Whether anything drives the pointer or the wheel, and so whether the
    /// tick loop has to sample the controller at all.
    pub fn drives_motion(&self) -> bool {
        self.directionals.iter().any(|p| !p.motions.is_empty())
            || self.triggers.iter().any(|t| !t.motions.is_empty())
    }
}

