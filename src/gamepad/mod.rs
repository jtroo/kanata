//! Game controller input at runtime.
//!
//! [`source`] is the only place that knows a controller library exists: it
//! owns a thread and sends key events straight to the processing loop, so a
//! controller button has no more latency than a key. [`PadEngine`] merges any
//! number of controllers into one logical pad -- a control is pressed while
//! any controller holds it and released when the last one lets go, which is a
//! bitwise OR of their [`PadSet`]s and one diff for the edges.
//!
//! Continuous output is *not* pushed. A stick driving the pointer is a rate,
//! not an event, so the tick loop samples it once a millisecond through
//! [`GamepadHandle::demand`]; pushing it would make pointer speed depend on
//! how often a particular controller happens to report.

pub mod source;

use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use kanata_parser::cfg::InputDeviceMatcher;
use kanata_parser::custom_action::{MWheelDirection, MoveDirection};
use kanata_parser::gamepad::{
    Demand, GamepadConfig, MotionKind, PadEdge, PadInput, PadSet, Projector, Vec2,
};
use parking_lot::Mutex;
use rustc_hash::FxHashMap as HashMap;

use crate::kanata::{CalculatedMouseMove, MAPPED_KEYS};
use crate::oskbd::{KeyEvent, KeyValue};

/// A backend-assigned controller identity, stable while the device stays
/// connected.
///
/// Deliberately opaque. An evdev node number or an XInput slot index is not a
/// stable identity, and letting one become one in a config would bake in a
/// value that changes when a device is replugged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PadDeviceId(u32);

impl PadDeviceId {
    pub const fn new(raw: u32) -> PadDeviceId {
        PadDeviceId(raw)
    }
}

/// What a backend knows about a controller when it connects.
#[derive(Clone, Debug, Default)]
pub struct PadDeviceInfo {
    pub name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

/// The result of offering a newly connected controller to the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    Accepted,
    /// The controller did not match the configured `definputdevices` entry.
    NotSelected,
}

/// Every connected controller, merged into one logical pad.
pub struct PadEngine {
    config: GamepadConfig,
    /// The `definputdevices` entry named by `(device N)`, if any.
    matcher: Option<InputDeviceMatcher>,
    pads: HashMap<PadDeviceId, (Projector, PadDeviceInfo)>,
    /// The union last handed to the layout.
    held: PadSet,
    /// Whether the last sample found the controllers asking for movement.
    moving: bool,
}

impl PadEngine {
    pub fn new(config: GamepadConfig, matcher: Option<InputDeviceMatcher>) -> PadEngine {
        PadEngine {
            config,
            matcher,
            pads: HashMap::default(),
            held: PadSet::EMPTY,
            moving: false,
        }
    }

    /// Whether the controllers have *just* started asking for movement.
    ///
    /// A control held at a deflection produces no edges, so a motion
    /// projection sends nothing down the input channel. The backend nudges the
    /// processing loop once per transition into movement; from there the tick
    /// loop keeps itself awake for as long as the demand lasts.
    ///
    /// The flag lives here rather than in the backend because the processing
    /// thread resets projectors behind the backend's back, on live reload and
    /// on a state clean. A copy cached out there would stay armed across that,
    /// and the next push would never wake the loop.
    pub fn take_motion_start(&mut self) -> bool {
        let moving = !self.demand().is_idle();
        let started = moving && !self.moving;
        self.moving = moving;
        started
    }

    pub fn connected_count(&self) -> usize {
        self.pads.len()
    }

    /// Human-readable description of a connected controller, for logs.
    pub fn device_name(&self, id: PadDeviceId) -> Option<&str> {
        self.pads.get(&id).map(|(_, info)| info.name.as_str())
    }

    /// Offer a newly connected controller to the engine.
    pub fn connect(&mut self, id: PadDeviceId, info: PadDeviceInfo) -> Connection {
        if let Some(matcher) = &self.matcher
            && !matches_device(matcher, &info)
        {
            return Connection::NotSelected;
        }
        // A backend replaying a connection is not an error; keep the state
        // that is already tracking the device.
        self.pads
            .entry(id)
            .or_insert_with(|| (Projector::new(self.config), info));
        Connection::Accepted
    }

    /// Drop a controller, releasing everything it was holding.
    ///
    /// Without this a controller unplugged mid-press leaves a key down with
    /// nothing left that could ever release it.
    pub fn disconnect(&mut self, id: PadDeviceId, edges: &mut Vec<PadEdge>) {
        if self.pads.remove(&id).is_some() {
            self.merge(edges);
        }
    }

    /// Feed one reading from a connected controller. Readings from a device
    /// the engine never accepted are dropped.
    pub fn feed(&mut self, id: PadDeviceId, input: PadInput, edges: &mut Vec<PadEdge>) {
        if let Some((projector, _)) = self.pads.get_mut(&id) {
            projector.feed(input);
            self.merge(edges);
        }
    }

    /// Swap in a new declaration across every connected controller.
    pub fn reconfigure(&mut self, config: GamepadConfig, edges: &mut Vec<PadEdge>) {
        self.config = config;
        self.moving = false;
        for (projector, _) in self.pads.values_mut() {
            projector.reconfigure(config);
        }
        self.merge(edges);
    }

    /// Release every control every controller is holding, keeping the
    /// controllers themselves connected.
    pub fn release_all(&mut self, edges: &mut Vec<PadEdge>) {
        self.moving = false;
        for (projector, _) in self.pads.values_mut() {
            projector.reset();
        }
        self.merge(edges);
    }

    /// Sum the motion demand of every controller.
    ///
    /// Summing rather than picking one keeps two controllers behaving like two
    /// hands on one pointer; an idle controller contributes nothing.
    pub fn demand(&self) -> Demand {
        self.pads
            .values()
            .map(|(projector, _)| projector.demand())
            .fold(Demand::default(), |total, demand| total + demand)
    }

    /// Republish the merged set, appending whatever transitions that implies.
    fn merge(&mut self, edges: &mut Vec<PadEdge>) {
        let next = self
            .pads
            .values()
            .fold(PadSet::EMPTY, |all, (projector, _)| all | projector.held());
        edges.extend(self.held.edges(next));
        self.held = next;
    }
}

/// Whether a controller satisfies a `definputdevices` matcher.
///
/// Every field the user specified has to match; fields they left out are not
/// constraints.
fn matches_device(matcher: &InputDeviceMatcher, info: &PadDeviceInfo) -> bool {
    matcher.name.as_ref().is_none_or(|name| *name == info.name)
        && matcher
            .vendor_id
            .is_none_or(|vendor| Some(vendor) == info.vendor_id)
        && matcher
            .product_id
            .is_none_or(|product| Some(product) == info.product_id)
}

/// Kanata's handle on the running controller backend.
///
/// Nothing here reaches the processing loop by itself. Every method that can
/// produce edges hands them back to the caller, because those callers run *on*
/// the processing thread: pushing releases down the input channel from there
/// would be a thread sending to itself, which can only be done with a
/// `try_send` that drops a release when the queue is full.
pub struct GamepadHandle {
    engine: Arc<Mutex<PadEngine>>,
    /// Whether anything drives the pointer or the wheel. Kept out here so the
    /// tick loop can skip sampling -- and so skip taking the engine lock a
    /// thousand times a second -- for the common keys-only configuration.
    drives_motion: bool,
}

impl GamepadHandle {
    /// Build the controller state machine, without a backend behind it.
    ///
    /// `devices` is the parsed `definputdevices` table, used to resolve the
    /// `(device N)` reference if the declaration has one.
    ///
    /// Separate from [`GamepadHandle::spawn_backend`] so everything downstream
    /// of the hardware can be driven from a recorded trace; the simulated
    /// input tests stand in for the backend that way.
    pub fn new(
        config: GamepadConfig,
        devices: Option<&[(std::num::NonZeroU8, InputDeviceMatcher)]>,
    ) -> GamepadHandle {
        GamepadHandle {
            drives_motion: config.drives_motion(),
            engine: Arc::new(Mutex::new(PadEngine::new(
                config,
                resolve_matcher(&config, devices),
            ))),
        }
    }

    /// Start the thread that reads real controllers into this engine.
    pub fn spawn_backend(&self, tx: SyncSender<KeyEvent>) {
        source::spawn(self.engine.clone(), tx);
    }

    /// Swap in a reloaded declaration, returning the edges the caller has to
    /// apply.
    ///
    /// Every control the old declaration held is released first, so a
    /// projection that disappears cannot strand a key down.
    ///
    /// Which controller is selected is fixed at startup and not revisited
    /// here, matching the existing rule that `definputdevices` is not re-read
    /// on live reload.
    pub fn reconfigure(&mut self, config: GamepadConfig, edges: &mut Vec<PadEdge>) {
        self.drives_motion = config.drives_motion();
        self.engine.lock().reconfigure(config, edges);
    }

    /// Let go of everything the controllers are holding, without changing what
    /// they project.
    ///
    /// `drives_motion` deliberately survives: it describes the declaration,
    /// not the held state.
    pub fn release_all(&mut self, edges: &mut Vec<PadEdge>) {
        self.engine.lock().release_all(edges);
    }

    /// Whether the tick loop has anything to sample.
    pub fn drives_motion(&self) -> bool {
        self.drives_motion
    }

    /// What the continuous projections want this tick.
    pub fn demand(&self) -> Demand {
        self.engine.lock().demand()
    }

    /// Offer a controller to the engine, as the backend does on connection.
    ///
    /// Public for the simulated-input tests, which have no hardware to
    /// connect; the backend thread owns the engine directly.
    pub fn connect(&self, id: PadDeviceId, info: PadDeviceInfo) -> Connection {
        self.engine.lock().connect(id, info)
    }

    /// Feed one reading from a connected controller, as the backend does.
    pub fn feed(&self, id: PadDeviceId, input: PadInput, edges: &mut Vec<PadEdge>) {
        self.engine.lock().feed(id, input, edges);
    }
}

fn resolve_matcher(
    config: &GamepadConfig,
    devices: Option<&[(std::num::NonZeroU8, InputDeviceMatcher)]>,
) -> Option<InputDeviceMatcher> {
    let wanted = config.device?;
    let (_, matcher) = devices?.iter().find(|(id, _)| *id == wanted)?;
    // `hash` is a keyboard-only concept. Reported rather than silently
    // ignored, because a config that looks like it selects one device but
    // actually accepts all of them is worse than a warning.
    if matcher.hash.is_some() {
        log::warn!(
            "gamepad: definputdevices entry {wanted} matches on `hash`, which controllers \
             do not report. Match on name, vendor_id or product_id instead."
        );
    }
    Some(matcher.clone())
}

/// Turn the edges a controller produced into input events for the processing
/// loop, dropping any control `defsrc` does not map.
///
/// This is the same gate the keyboard path uses. There is no passthrough
/// branch because the controller was never seized: the OS already saw the
/// original event.
///
/// Filtering is separated from delivery so that no caller holds the
/// `MAPPED_KEYS` lock while doing anything that can block. A live reload
/// replaces `MAPPED_KEYS` from the thread that drains the input channel, so a
/// send under this lock would let the two wait on each other.
pub(crate) fn gate_on_defsrc(edges: &[PadEdge], out: &mut Vec<KeyEvent>) {
    if edges.is_empty() {
        return;
    }
    let mapped = MAPPED_KEYS.lock();
    out.extend(edges.iter().filter_map(|edge| {
        let code = edge.code.os_code();
        mapped.contains(&code).then(|| {
            let value = match edge.pressed {
                true => KeyValue::Press,
                false => KeyValue::Release,
            };
            KeyEvent::new(code, value)
        })
    }));
}

/// Carries fractional pointer and wheel movement between ticks.
///
/// A stick at 30% of a 25,000 px/s speed asks for 7.5 pixels a millisecond.
/// Rounding each tick independently would run permanently slow or fast, and
/// rounding a small demand to zero would make slow movement impossible rather
/// than merely slow.
#[derive(Default)]
pub struct Accumulator([Vec2; MotionKind::ALL.len()]);

impl Accumulator {
    /// Add this tick's demand and take whatever whole units have accrued.
    pub fn accrue(&mut self, demand: Demand) -> Accrued {
        let mut accrued = Accrued::default();
        for kind in MotionKind::ALL {
            let carry = &mut self.0[kind as usize];
            *carry += demand[kind];
            accrued.0[kind as usize] = carry.take_whole();
        }
        accrued
    }

    /// Drop any partial movement.
    ///
    /// Called when everything returns to rest, so a fraction left over from
    /// the last push cannot leak into the next one as a stray pixel.
    pub fn reset(&mut self) {
        *self = Accumulator::default();
    }
}

/// Whole units ready to send to the OS this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Accrued([(i32, i32); MotionKind::ALL.len()]);

impl Accrued {
    pub fn is_empty(self) -> bool {
        self.0.iter().all(|axes| *axes == (0, 0))
    }

    /// The pointer moves this tick, as kanata's mouse primitives take them.
    ///
    /// One entry per axis, `None` for an axis that is not moving; the caller
    /// picks `move_mouse` or `move_mouse_many` from what is present. An array
    /// rather than a `Vec` keeps the per-millisecond tick path allocation
    /// free.
    pub fn mouse_moves(self, scale: impl Fn(u16) -> u16) -> [Option<CalculatedMouseMove>; 2] {
        let (x, y) = self.0[MotionKind::Mouse as usize];
        [
            split(x, MoveDirection::Right, MoveDirection::Left),
            split(y, MoveDirection::Down, MoveDirection::Up),
        ]
        .map(|axis| {
            axis.map(|(direction, distance)| CalculatedMouseMove {
                direction,
                distance: scale(distance),
            })
        })
    }

    /// The wheel notches this tick, as (direction, distance) pairs.
    pub fn scrolls(self) -> [Option<(MWheelDirection, u16)>; 2] {
        let (x, y) = self.0[MotionKind::Scroll as usize];
        [
            split(x, MWheelDirection::Right, MWheelDirection::Left),
            // Stick Y and wheel-up are both up-positive, so no flip here. The
            // user-facing invert-y knob was applied in the projection.
            split(y, MWheelDirection::Up, MWheelDirection::Down),
        ]
    }
}

/// Split a signed amount into a direction and a distance, or `None` for an
/// axis that is not moving. Saturating, so an absurd demand cannot wrap.
fn split<T>(amount: i32, positive: T, negative: T) -> Option<(T, u16)> {
    match amount {
        0 => None,
        _ => Some((
            if amount > 0 { positive } else { negative },
            amount.unsigned_abs().min(u16::MAX as u32) as u16,
        )),
    }
}
