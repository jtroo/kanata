//! Projection: controller readings in, the set of held controls out.
//!
//! Every reading updates [`PadState`] -- what the controller has told us,
//! untransformed -- and then the whole [`PadSet`] is recomputed from it. That
//! is a handful of comparisons over one `u64`, and it removes the class of bug
//! where one control group's edge logic disagrees with another's: one place
//! decides what is held, and the caller diffs the result.
//!
//! Two unifications live here because both are invisible to the user and
//! neither belongs in a backend. A **trigger** is reported by most pads twice,
//! as a digital button and as an analog axis; a **d-pad** is reported either
//! as four contacts or as a hat axis. Both halves are kept and unioned, so a
//! pad reporting one still works and a pad reporting both does not
//! double-fire.

use super::analog::{Demand, StickPosition, Unit};
use super::{
    Cardinal, CardinalSet, Directional, GamepadConfig, PadButton, PadCode, PadSet, Side, Socd,
};

/// One reading from a controller, already normalized by the backend.
///
/// Axis values are y-up and in `[-1, 1]`; trigger values are in `[0, 1]`.
/// Multi-axis controls arrive whole rather than one axis at a time, so a
/// diagonal push never momentarily looks like a cardinal one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadInput {
    /// A digital control the backend could name.
    Button {
        button: PadButton,
        pressed: bool,
    },
    Stick {
        side: Side,
        value: StickPosition,
    },
    /// A trigger's analog half.
    Trigger {
        side: Side,
        value: Unit,
    },
    /// The d-pad, as its four independent contacts.
    Contacts(CardinalSet),
    /// The d-pad, as a hat axis.
    Hat(CardinalSet),
    /// A control with no portable name, identified by its backend code.
    Native {
        code: u32,
        pressed: bool,
    },
}

/// Everything a controller has told us, untransformed.
///
/// Raw on purpose: every consumer applies its own transformations, so no site
/// has to ask which ones have already happened. Kept even for controls with
/// no projection, so a live reload can begin using one without waiting for
/// the user to move it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    /// One bit per [`PadButton`].
    buttons: u16,
    /// The d-pad from its contacts and from its hat, kept apart so one going
    /// quiet cannot clear the other.
    dpad: [CardinalSet; 2],
    sticks: [StickPosition; Side::ALL.len()],
    triggers: [Unit; Side::ALL.len()],
    /// Slots already resolved from backend codes, so the projection does not
    /// have to consult the bindings again.
    slots: PadSet,
}

impl PadState {
    fn apply(&mut self, input: PadInput, config: &GamepadConfig) {
        match input {
            PadInput::Button { button, pressed } => {
                let bit = 1u16 << button as u16;
                self.buttons = match pressed {
                    true => self.buttons | bit,
                    false => self.buttons & !bit,
                };
            }
            PadInput::Stick { side, value } => self.sticks[side as usize] = value,
            PadInput::Trigger { side, value } => self.triggers[side as usize] = value,
            PadInput::Contacts(dirs) => self.dpad[0] = dirs,
            PadInput::Hat(dirs) => self.dpad[1] = dirs,
            // An unbound code is dropped here. The backend logs it so it can
            // be discovered and bound; letting it through would mean every
            // paddle and vendor button claiming a control nobody asked for.
            PadInput::Native { code, pressed } => {
                if let Some(slot) = config.slot_of(code) {
                    self.slots.set(slot, pressed);
                }
            }
        }
    }

    fn holds(&self, button: PadButton) -> bool {
        self.buttons & (1 << button as u16) != 0
    }

    /// The d-pad's contacts, before arbitration.
    fn dpad(&self) -> CardinalSet {
        self.dpad[0] | self.dpad[1]
    }
}

/// Which direction of each opposed pair arrived most recently.
///
/// `Last` and `First` are history-dependent, which is the only reason
/// resolution needs state at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SocdMemory {
    previous: CardinalSet,
    newest: [Option<Cardinal>; 2],
}

/// The two opposed pairs, positive direction first so that `Positive` and
/// `Negative` have one place to consult.
const PAIRS: [(Cardinal, Cardinal); 2] = [
    (Cardinal::Up, Cardinal::Down),
    (Cardinal::Right, Cardinal::Left),
];

impl SocdMemory {
    /// Resolve one set of cardinals into the set that should reach the layout.
    ///
    /// The mode applies independently to the vertical and horizontal pairs, so
    /// a genuine diagonal is never disturbed: only an opposed axis is
    /// rewritten.
    fn resolve(&mut self, mode: Socd, raw: CardinalSet) -> CardinalSet {
        let mut resolved = raw;
        for (axis, (positive, negative)) in PAIRS.into_iter().enumerate() {
            // Track which direction arrived most recently whether or not the
            // axis is opposed right now: the press that creates the conflict
            // may be the one that arrives second.
            let positive_new = raw.contains(positive) && !self.previous.contains(positive);
            let negative_new = raw.contains(negative) && !self.previous.contains(negative);
            self.newest[axis] = match (positive_new, negative_new) {
                (true, false) => Some(positive),
                (false, true) => Some(negative),
                // A snapshot in which both contacts first appear has no
                // temporal ordering. Picking one based on enum iteration
                // would pretend to know which was last (and make `first`
                // pretend the opposite), so history-dependent modes resolve
                // it neutrally until a later transition establishes order.
                (true, true) => None,
                (false, false) => self.newest[axis],
            };
            if !(raw.contains(positive) && raw.contains(negative)) {
                continue;
            }
            // With exactly two directions on the axis, the incumbent is
            // whichever one is not the newcomer.
            let winner = match mode {
                Socd::Off => continue,
                Socd::Neutral => None,
                Socd::Positive => Some(positive),
                Socd::Negative => Some(negative),
                Socd::Last => self.newest[axis],
                Socd::First => self.newest[axis].map(Cardinal::opposite),
            };
            resolved.set(positive, false);
            resolved.set(negative, false);
            if let Some(winner) = winner {
                resolved.set(winner, true);
            }
        }
        self.previous = raw;
        resolved
    }
}

/// Turns one controller's readings into a set of held controls and a motion
/// demand.
///
/// One projector per controller, so a stick on one pad cannot cancel a stick
/// on another. The runtime merges them.
#[derive(Clone, Debug)]
pub struct Projector {
    config: GamepadConfig,
    state: PadState,
    /// Only the d-pad has independent contacts, so it is the only control that
    /// can assert both directions of an axis and the only one with anything to
    /// arbitrate.
    socd: SocdMemory,
    /// The d-pad's contacts *after* arbitration. Both halves of its projection
    /// read this one value: the digital half presses these directions and the
    /// motion half points along them, so `(socd positive)` cannot make them
    /// disagree about which way the pad is pointing.
    dpad: CardinalSet,
    held: PadSet,
}

impl Projector {
    pub fn new(config: GamepadConfig) -> Projector {
        Projector {
            config,
            state: PadState::default(),
            socd: SocdMemory::default(),
            dpad: CardinalSet::EMPTY,
            held: PadSet::EMPTY,
        }
    }

    /// The controls this controller is holding.
    pub fn held(&self) -> PadSet {
        self.held
    }

    /// Feed one reading and get the new held set.
    pub fn feed(&mut self, input: PadInput) -> PadSet {
        self.state.apply(input, &self.config);
        self.recompute()
    }

    /// Let go of everything and forget all history.
    ///
    /// Used on disconnect, on live reload and on shutdown, where continuing to
    /// trust the old state would strand a key down. Recomputing from an empty
    /// [`PadState`] is what makes this correct by construction rather than by
    /// remembering to release each control group.
    pub fn reset(&mut self) {
        *self = Projector::new(self.config);
    }

    /// Swap in a reloaded declaration, keeping nothing.
    pub fn reconfigure(&mut self, config: GamepadConfig) {
        *self = Projector::new(config);
    }

    /// What the continuous projections want this tick.
    ///
    /// Sampled rather than pushed: a deflection is a rate, so how fast the
    /// pointer moves should depend on the clock and not on how often a
    /// particular controller happens to report.
    pub fn demand(&self) -> Demand {
        let mut demand = Demand::default();
        for control in Directional::ALL {
            for kind in super::MotionKind::ALL {
                let Some(motion) = self.config.projection(control).motions.get(kind) else {
                    continue;
                };
                // A d-pad points along its arbitrated contacts, so it reads as
                // a stick at full deflection and the two halves of its
                // projection agree about which way it is pointing.
                let pointing = match control.side() {
                    Some(side) => self.state.sticks[side as usize].vector(),
                    None => self.dpad.vector(),
                };
                demand[kind] += motion.displace(pointing);
            }
        }
        for side in Side::ALL {
            for kind in super::MotionKind::ALL {
                let Some(projected) = self.config.trigger(side).motions.get(kind) else {
                    continue;
                };
                let rate = projected
                    .motion
                    .rate(self.state.triggers[side as usize].get());
                demand[kind] += projected.direction.vector() * rate;
            }
        }
        demand
    }

    /// Rebuild the held set from the raw state.
    fn recompute(&mut self) -> PadSet {
        let mut set = self.state.slots;
        for button in PadButton::ALL {
            set.set(PadCode::button(button), self.state.holds(button));
        }
        for side in Side::ALL {
            if self.state.triggers[side as usize] > self.config.trigger(side).threshold {
                set.insert(PadCode::button(side.trigger()));
            }
        }
        // Arbitrated before the loop, and unconditionally, because `demand`
        // reads the result too: a d-pad with a motion projection and no
        // digital one still has to point somewhere.
        let dpad = self.config.projection(Directional::Dpad).digital;
        let socd = dpad.map_or(Socd::default(), |digital| digital.socd);
        self.dpad = self.socd.resolve(socd, self.state.dpad());

        for control in Directional::ALL {
            let Some(digital) = self.config.projection(control).digital else {
                continue;
            };
            let dirs = match control.side() {
                Some(side) => self.threshold(side, digital.threshold),
                None => self.dpad,
            };
            for dir in dirs.projected(digital.mode) {
                set.insert(PadCode::direction(control, dir));
            }
        }
        self.held = set;
        set
    }

    /// The cardinals a stick is asserting.
    ///
    /// Compared component by component rather than by radius -- a stick pushed
    /// fully right is at full radius but is not pressing Up -- and against the
    /// *raw* reading, so `(threshold 0.5)` means half of physical deflection and a
    /// deadzone cannot silently move it.
    fn threshold(&self, side: Side, threshold: Unit) -> CardinalSet {
        let value = self.state.sticks[side as usize];
        let mut out = CardinalSet::EMPTY;
        let components = [
            value.y().get(),
            -value.y().get(),
            -value.x().get(),
            value.x().get(),
        ];
        for (dir, magnitude) in Cardinal::ALL.into_iter().zip(components) {
            out.set(dir, Unit::new(magnitude) > threshold);
        }
        out
    }
}

