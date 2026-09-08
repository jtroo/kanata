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
    /// Analog controls that have crossed their threshold and stayed there.
    stable_analog: PadSet,
    /// A pending change for each analog control. A change only becomes stable
    /// after its configured grace period has elapsed uninterrupted.
    pending: [Option<Pending>; PadCode::COUNT],
    held: PadSet,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    pressed: bool,
    remaining: u16,
}

impl Projector {
    pub fn new(config: GamepadConfig) -> Projector {
        Projector {
            config,
            state: PadState::default(),
            socd: SocdMemory::default(),
            dpad: CardinalSet::EMPTY,
            stable_analog: PadSet::EMPTY,
            pending: [None; PadCode::COUNT],
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

    /// Advance pending threshold crossings by `milliseconds`.
    ///
    /// The processing loop calls this once per millisecond, like Kanata's
    /// other waiting states. A late tick is still correct because elapsed time
    /// is subtracted rather than counted as one controller event.
    pub fn tick(&mut self, milliseconds: u16) -> PadSet {
        for (index, pending) in self.pending.iter_mut().enumerate() {
            let Some(change) = pending.as_mut() else {
                continue;
            };
            change.remaining = change.remaining.saturating_sub(milliseconds);
            if change.remaining == 0 {
                self.stable_analog
                    .set(PadCode::from_index(index), change.pressed);
                *pending = None;
            }
        }
        self.compose_held()
    }

    /// Whether a threshold crossing still needs the tick loop to run.
    pub fn has_pending(&self) -> bool {
        self.pending.iter().any(Option::is_some)
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
        let mut direct = self.state.slots;
        for button in PadButton::ALL {
            direct.set(PadCode::button(button), self.state.holds(button));
        }
        // Arbitrated before the loop, and unconditionally, because `demand`
        // reads the result too: a d-pad with a motion projection and no
        // digital one still has to point somewhere.
        let dpad = self.config.projection(Directional::Dpad).digital;
        let socd = dpad.map_or(Socd::default(), |digital| digital.socd);
        self.dpad = self.socd.resolve(socd, self.state.dpad());

        let mut desired_analog = PadSet::EMPTY;
        for side in Side::ALL {
            let trigger = self.config.trigger(side);
            let code = PadCode::button(side.trigger());
            desired_analog.set(code, self.state.triggers[side as usize] > trigger.threshold);
            self.reconcile(code, desired_analog.contains(code), trigger.debounce);
        }
        for control in Directional::ALL {
            let Some(digital) = self.config.projection(control).digital else {
                continue;
            };
            let dirs = match control.side() {
                Some(side) => self.threshold(side, digital.threshold),
                None => self.dpad,
            };
            for dir in dirs.projected(digital.mode) {
                let code = PadCode::direction(control, dir);
                match control.side() {
                    Some(_) => desired_analog.insert(code),
                    None => direct.insert(code),
                }
            }
            if control.side().is_some() {
                for dir in super::Dir::ALL {
                    let code = PadCode::direction(control, dir);
                    self.reconcile(code, desired_analog.contains(code), digital.debounce);
                }
            }
        }
        self.compose_held_from(direct)
    }

    fn reconcile(&mut self, code: PadCode, pressed: bool, debounce: u16) {
        if self.stable_analog.contains(code) == pressed {
            self.pending[code.0 as usize] = None;
        } else if debounce == 0 {
            self.stable_analog.set(code, pressed);
            self.pending[code.0 as usize] = None;
        } else if self.pending[code.0 as usize].is_none_or(|pending| pending.pressed != pressed) {
            self.pending[code.0 as usize] = Some(Pending {
                pressed,
                remaining: debounce,
            });
        }
    }

    fn compose_held_from(&mut self, direct: PadSet) -> PadSet {
        self.held = direct | self.stable_analog;
        self.held
    }

    fn compose_held(&mut self) -> PadSet {
        let mut direct = self.state.slots;
        for button in PadButton::ALL {
            direct.set(PadCode::button(button), self.state.holds(button));
        }
        if let Some(digital) = self.config.projection(Directional::Dpad).digital {
            for dir in self.dpad.projected(digital.mode) {
                direct.insert(PadCode::direction(Directional::Dpad, dir));
            }
        }
        self.compose_held_from(direct)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepad::analog::{Curve, Motion, MotionKind, Vec2};
    use crate::gamepad::{Digital, Dir, DirMode, PadControl};

    /// Drive a trace and report what is held afterwards, plus every edge as
    /// `(code, pressed)`.
    fn run(config: GamepadConfig, trace: &[PadInput]) -> (Vec<PadCode>, Vec<(PadCode, bool)>) {
        let mut projector = Projector::new(config);
        let mut edges = Vec::new();
        for input in trace {
            let before = projector.held();
            edges.extend(
                before
                    .edges(projector.feed(*input))
                    .map(|e| (e.code, e.pressed)),
            );
        }
        (projector.held().iter().collect(), edges)
    }

    fn held(config: GamepadConfig, trace: &[PadInput]) -> Vec<PadCode> {
        run(config, trace).0
    }

    fn digital(control: Directional, digital: Digital) -> GamepadConfig {
        let mut config = GamepadConfig::default();
        config.projection_mut(control).digital = Some(digital);
        config
    }

    fn eight_way(control: Directional) -> GamepadConfig {
        digital(
            control,
            Digital {
                mode: DirMode::EightWay,
                ..Digital::default()
            },
        )
    }

    fn stick(x: f32, y: f32) -> PadInput {
        PadInput::Stick {
            side: Side::Left,
            value: StickPosition::new(x, y),
        }
    }

    fn trigger(side: Side, value: f32) -> PadInput {
        PadInput::Trigger {
            side,
            value: Unit::new(value),
        }
    }

    fn fast(speed: f32) -> Motion {
        Motion {
            deadzone: Unit::ZERO,
            speed: speed * 1000.0,
            curve: Curve::Linear,
            invert: Vec2::KEEP,
        }
    }

    fn lstick(dir: Dir) -> PadCode {
        PadCode::direction(Directional::LeftStick, dir)
    }

    fn dpad(dir: Dir) -> PadCode {
        PadCode::direction(Directional::Dpad, dir)
    }

    fn button(button: PadButton, pressed: bool) -> PadInput {
        PadInput::Button { button, pressed }
    }

    #[test]
    fn a_press_is_one_edge_and_a_repeat_is_none() {
        let south = PadCode::button(PadButton::South);
        let (held, edges) = run(
            GamepadConfig::default(),
            &[
                button(PadButton::South, true),
                button(PadButton::South, true),
                button(PadButton::South, false),
                button(PadButton::South, false),
            ],
        );
        assert!(held.is_empty());
        assert_eq!(edges, vec![(south, true), (south, false)]);
    }

    #[test]
    fn a_trigger_reported_as_both_a_button_and_an_axis_fires_once() {
        // Most pads report a trigger twice. Firing on each would double every
        // press; firing on neither when only one arrives would drop it.
        let l2 = vec![PadCode::button(PadButton::L2)];
        let cfg = GamepadConfig::default();
        assert_eq!(held(cfg, &[trigger(Side::Left, 1.0)]), l2, "axis alone");
        assert_eq!(
            held(cfg, &[button(PadButton::L2, true)]),
            l2,
            "button alone"
        );
        assert_eq!(
            run(
                cfg,
                &[
                    button(PadButton::L2, true),
                    trigger(Side::Left, 1.0),
                    trigger(Side::Left, 0.0),
                    button(PadButton::L2, false),
                ]
            )
            .1
            .len(),
            2,
            "together they must still be one press and one release"
        );

        // A slow squeeze crosses the band once each way rather than chattering.
        let squeeze: Vec<_> = (0..=100)
            .chain((0..100).rev())
            .map(|i| trigger(Side::Right, i as f32 / 100.0))
            .collect();
        let (held, edges) = run(cfg, &squeeze);
        assert!(held.is_empty());
        assert_eq!(edges.len(), 2, "squeeze chattered: {edges:?}");
    }

    #[test]
    fn four_way_presses_both_cardinals_and_eight_way_presses_the_diagonal() {
        let four = digital(Directional::LeftStick, Digital::default());
        assert_eq!(
            held(four, &[stick(0.8, 0.8)]),
            vec![lstick(Dir::Up), lstick(Dir::Right)]
        );
        let eight = eight_way(Directional::LeftStick);
        assert_eq!(held(eight, &[stick(0.8, 0.8)]), vec![lstick(Dir::UpRight)]);
        // A straight push still presses its cardinal, so all eight are usable.
        assert_eq!(held(eight, &[stick(0.0, 0.8)]), vec![lstick(Dir::Up)]);
    }

    #[test]
    fn a_stick_never_asserts_both_directions_of_an_axis() {
        // Each axis is one signed number, so there is nothing for SOCD to
        // arbitrate on a stick -- which is why the mode is inert there.
        let config = digital(Directional::LeftStick, Digital::default());
        for step in -12i32..=12 {
            let (x, y) = (step as f32 / 8.0, (12 - step.abs()) as f32 / 8.0);
            let set = Projector::new(config).feed(stick(x, y));
            for (a, b) in [(Dir::Up, Dir::Down), (Dir::Left, Dir::Right)] {
                assert!(
                    !(set.contains(lstick(a)) && set.contains(lstick(b))),
                    "({x}, {y}) asserted both of {a:?}/{b:?}"
                );
            }
        }
    }

    #[test]
    fn a_stick_moving_between_cardinals_releases_before_it_presses() {
        // Holding both for even one event is a diagonal as far as a game is
        // concerned.
        let config = digital(Directional::LeftStick, Digital::default());
        assert_eq!(
            run(config, &[stick(0.0, 1.0), stick(-1.0, 0.0)]).1,
            vec![
                (lstick(Dir::Up), true),
                (lstick(Dir::Up), false),
                (lstick(Dir::Left), true),
            ]
        );
    }

    #[test]
    fn a_stick_resting_exactly_on_its_threshold_does_not_chatter() {
        let config = digital(Directional::LeftStick, Digital::default());
        let threshold = Digital::default().threshold.get();
        let trace: Vec<_> = (0..500).map(|_| stick(0.0, threshold)).collect();
        assert!(
            run(config, &trace).1.is_empty(),
            "a threshold rested at its boundary must stay quiet"
        );
    }

    #[test]
    fn debounce_requires_an_uninterrupted_threshold_crossing() {
        let config = digital(
            Directional::LeftStick,
            Digital {
                debounce: 3,
                ..Digital::default()
            },
        );
        let mut projector = Projector::new(config);
        let up = lstick(Dir::Up);

        assert!(projector.feed(stick(0.0, 1.0)).is_empty());
        assert!(projector.has_pending());
        assert!(projector.tick(2).is_empty(), "two milliseconds is too soon");
        assert!(projector.tick(1).contains(up));

        // A brief dip below the threshold starts a release, but returning to
        // the stable side cancels it before it can create an edge.
        projector.feed(stick(0.0, 0.0));
        assert!(projector.has_pending());
        assert!(projector.feed(stick(0.0, 1.0)).contains(up));
        assert!(!projector.has_pending());

        projector.feed(stick(0.0, 0.0));
        assert!(projector.tick(2).contains(up));
        assert!(projector.tick(1).is_empty());
    }

    #[test]
    fn a_dpad_needs_no_declaration_and_unifies_its_two_reportings() {
        // It is digital hardware, and controllers report it either as four
        // contacts or as a hat axis depending on the driver.
        let (held, edges) = run(
            GamepadConfig::default(),
            &[
                PadInput::Contacts(CardinalSet::of(&[Cardinal::Up])),
                PadInput::Hat(CardinalSet::of(&[Cardinal::Up])),
                PadInput::Contacts(CardinalSet::EMPTY),
            ],
        );
        assert_eq!(edges.len(), 1, "the second source re-pressed: {edges:?}");
        assert_eq!(held, vec![dpad(Dir::Up)]);

        // And it takes the sticks' eight-way mode, because it is the same kind
        // of control.
        assert_eq!(
            self::held(
                eight_way(Directional::Dpad),
                &[PadInput::Hat(CardinalSet::of(&[
                    Cardinal::Down,
                    Cardinal::Left,
                ]))]
            ),
            vec![dpad(Dir::DownLeft)]
        );
    }

    /// The d-pad directions held after driving a contact trace.
    fn socd_after(mode: Socd, trace: &[(Dir, bool)]) -> Vec<Dir> {
        let config = digital(
            Directional::Dpad,
            Digital {
                socd: mode,
                ..Digital::default()
            },
        );
        let mut contacts = CardinalSet::EMPTY;
        let inputs: Vec<_> = trace
            .iter()
            .map(|(dir, pressed)| {
                contacts.set(
                    Cardinal::try_from(*dir).expect("test trace uses cardinals"),
                    *pressed,
                );
                PadInput::Contacts(contacts)
            })
            .collect();
        held(config, &inputs)
            .into_iter()
            .map(|code| match code.control() {
                PadControl::Direction(Directional::Dpad, dir) => dir,
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn socd_resolves_an_opposed_axis_the_way_the_mode_says() {
        let opposed = [(Dir::Left, true), (Dir::Right, true)];
        for (mode, expected) in [
            (Socd::Off, vec![Dir::Left, Dir::Right]),
            (Socd::Neutral, vec![]),
            (Socd::Last, vec![Dir::Right]),
            (Socd::First, vec![Dir::Left]),
            (Socd::Positive, vec![Dir::Right]),
            (Socd::Negative, vec![Dir::Left]),
        ] {
            assert_eq!(socd_after(mode, &opposed), expected, "{mode:?}");
        }
        // `last` and `first` follow the order the contacts arrived in, which is
        // why resolution keeps state at all.
        let reversed = [
            (Dir::Left, true),
            (Dir::Right, true),
            (Dir::Right, false),
            (Dir::Left, false),
            (Dir::Right, true),
            (Dir::Left, true),
        ];
        assert_eq!(socd_after(Socd::Last, &reversed), vec![Dir::Left]);
        assert_eq!(socd_after(Socd::First, &reversed), vec![Dir::Right]);
    }

    #[test]
    fn simultaneous_opposites_do_not_invent_an_order() {
        let both = PadInput::Contacts(CardinalSet::of(&[Cardinal::Left, Cardinal::Right]));
        for mode in [Socd::Last, Socd::First] {
            let config = digital(
                Directional::Dpad,
                Digital {
                    socd: mode,
                    ..Digital::default()
                },
            );
            assert!(
                held(config, &[both]).is_empty(),
                "{mode:?} invented a winner"
            );
        }
    }

    #[test]
    fn socd_only_rewrites_the_axis_that_is_opposed() {
        // A genuine diagonal is an answer, not a conflict.
        for mode in [Socd::Neutral, Socd::Last, Socd::First, Socd::Positive] {
            assert_eq!(
                socd_after(mode, &[(Dir::Up, true), (Dir::Right, true)]),
                vec![Dir::Up, Dir::Right],
                "{mode:?} disturbed a diagonal"
            );
        }
        assert_eq!(
            socd_after(
                Socd::Neutral,
                &[(Dir::Up, true), (Dir::Left, true), (Dir::Right, true)]
            ),
            vec![Dir::Up],
            "the unopposed axis should survive"
        );
    }

    #[test]
    fn a_reset_or_a_reload_releases_everything_and_forgets_the_history() {
        let config = digital(Directional::LeftStick, Digital::default());
        let mut projector = Projector::new(config);
        projector.feed(button(PadButton::South, true));
        let before = projector.feed(stick(1.0, 0.0));
        assert!(!before.is_empty());

        projector.reset();
        assert!(projector.held().is_empty());
        assert!(
            before.edges(projector.held()).all(|edge| !edge.pressed),
            "a reset may only release"
        );
        // The stick has to re-cross its threshold rather than resume held.
        assert!(projector.feed(stick(0.4, 0.0)).is_empty());

        // A reload to a declaration without that stick is the same, plus the
        // stick stops projecting: this is what a deleted `defgamepad` lands on.
        projector.feed(stick(1.0, 0.0));
        projector.reconfigure(GamepadConfig::default());
        assert!(projector.held().is_empty());
        assert!(projector.feed(stick(1.0, 1.0)).is_empty());
    }

    #[test]
    fn only_a_bound_backend_code_reaches_a_slot() {
        let mut config = GamepadConfig::default();
        config.slots[2] = Some(0x2c0);
        let press = |code| PadInput::Native {
            code,
            pressed: true,
        };
        assert_eq!(
            held(config, &[press(0x2c0)]),
            vec![PadCode::slot(2).expect("a slot")]
        );
        assert!(held(config, &[press(0x2c1)]).is_empty(), "unbound code");
    }

    #[test]
    fn every_kind_of_control_can_drive_motion() {
        // The threshold and the displacement read the same value without
        // disturbing each other, so a stick may do both; a d-pad reads as a
        // stick at full deflection; and a trigger is a scalar pushed along the
        // direction it was given.
        let mut config = digital(Directional::LeftStick, Digital::default());
        config
            .projection_mut(Directional::LeftStick)
            .motions
            .set(MotionKind::Mouse, fast(10.0));
        config
            .projection_mut(Directional::LeftStick)
            .motions
            .set(MotionKind::Scroll, fast(2.0));
        config
            .projection_mut(Directional::Dpad)
            .motions
            .set(MotionKind::Mouse, fast(6.0));
        config.trigger_mut(Side::Right).motions.set(
            MotionKind::Scroll,
            crate::gamepad::TriggerMotion {
                direction: Cardinal::Down,
                motion: fast(4.0),
            },
        );

        let mut projector = Projector::new(config);
        assert!(projector.demand().is_idle());

        let held = projector.feed(stick(1.0, 0.0)).iter().collect::<Vec<_>>();
        assert_eq!(held, vec![lstick(Dir::Right)], "the stick still presses");
        assert!((projector.demand()[MotionKind::Mouse].x - 10.0).abs() < 1e-3);
        assert!((projector.demand()[MotionKind::Scroll].x - 2.0).abs() < 1e-3);

        projector.feed(PadInput::Hat(CardinalSet::of(&[Cardinal::Right])));
        assert!((projector.demand()[MotionKind::Mouse].x - 16.0).abs() < 1e-3);

        projector.feed(trigger(Side::Right, 0.5));
        assert_eq!(
            projector.demand()[MotionKind::Scroll],
            Vec2 { x: 2.0, y: -2.0 }
        );
    }
}
