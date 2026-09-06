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
