//! The controller backend: gilrs events in, [`PadInput`] out.
//!
//! The only file that knows a controller library exists. It runs on its own
//! thread because `gilrs::Gilrs` is not `Send` on every platform, so the
//! context is built inside the thread and only translated events leave it. One
//! backend covers Linux (evdev), macOS (IOKit) and Windows (XInput), which is
//! why there is not a single `#[cfg]` below.
//!
//! Worth stating plainly rather than leaving to be discovered: **the
//! controller is not seized.** Games and the desktop still see the pad, so
//! kanata cannot suppress the original input the way it does for a grabbed
//! keyboard.

use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

use gilrs::{Axis, Button, Event, EventType, GilrsBuilder};
use kanata_parser::gamepad::{
    Cardinal, CardinalSet, PadButton, PadEdge, PadInput, Side, StickPosition, Unit,
};
use kanata_parser::keys::OsCode;
use parking_lot::Mutex;
use rustc_hash::FxHashMap as HashMap;

use super::{Connection, PadDeviceId, PadDeviceInfo, PadEngine, gate_on_defsrc};
use crate::oskbd::{KeyEvent, KeyValue};

/// How long to wait for a controller event before looping.
///
/// Only exists so the thread can notice that the processing loop has gone away
/// and exit, instead of blocking forever on a channel nobody reads.
const POLL_TIMEOUT: Duration = Duration::from_millis(500);

/// What a gilrs button name is, in kanata's vocabulary.
///
/// Both vocabularies are positional, so this is a rename rather than a
/// remapping: gilrs's `South` and kanata's `pad-south` are the same physical
/// control on every controller.
enum Control {
    Button(PadButton),
    /// A d-pad contact, which is a direction rather than a button.
    Contact(Cardinal),
    /// A control gilrs could not name. It reaches the layout only if the user
    /// bound its backend code to a slot in `defgamepad`.
    Unnamed,
}

fn classify(button: Button) -> Control {
    match button {
        Button::South => Control::Button(PadButton::South),
        Button::East => Control::Button(PadButton::East),
        Button::West => Control::Button(PadButton::West),
        Button::North => Control::Button(PadButton::North),
        Button::C => Control::Button(PadButton::C),
        Button::Z => Control::Button(PadButton::Z),
        Button::LeftTrigger => Control::Button(PadButton::L1),
        Button::RightTrigger => Control::Button(PadButton::R1),
        Button::LeftTrigger2 => Control::Button(PadButton::L2),
        Button::RightTrigger2 => Control::Button(PadButton::R2),
        Button::Select => Control::Button(PadButton::Select),
        Button::Start => Control::Button(PadButton::Start),
        Button::Mode => Control::Button(PadButton::Mode),
        Button::LeftThumb => Control::Button(PadButton::L3),
        Button::RightThumb => Control::Button(PadButton::R3),
        Button::DPadUp => Control::Contact(Cardinal::Up),
        Button::DPadDown => Control::Contact(Cardinal::Down),
        Button::DPadLeft => Control::Contact(Cardinal::Left),
        Button::DPadRight => Control::Contact(Cardinal::Right),
        // Deliberately exhaustive: if gilrs grows a button this should stop
        // compiling rather than silently drop the control.
        Button::Unknown => Control::Unnamed,
    }
}

/// The stick a gilrs axis belongs to, if it is one. Triggers and extra axes
/// are reported through their own events.
fn stick_side(axis: Axis) -> Option<Side> {
    match axis {
        Axis::LeftStickX | Axis::LeftStickY => Some(Side::Left),
        Axis::RightStickX | Axis::RightStickY => Some(Side::Right),
        _ => None,
    }
}

/// The trigger an analog button report belongs to, if it is one.
fn trigger_side(button: Button) -> Option<Side> {
    match button {
        Button::LeftTrigger2 => Some(Side::Left),
        Button::RightTrigger2 => Some(Side::Right),
        _ => None,
    }
}

/// Per-controller latching the backend itself needs, as distinct from the
/// projection state the engine holds.
///
/// A stick reports its axes one at a time and a d-pad reports one contact at a
/// time, but a projection needs the whole reading: fed piecemeal, a diagonal
/// push briefly looks like a cardinal one and emits a spurious edge. Latching
/// the most recent value of each is what makes a whole reading reach the
/// projector.
#[derive(Default)]
struct Pad {
    sticks: [StickPosition; 2],
    hat: CardinalSet,
    contacts: CardinalSet,
}

/// Translate one backend event into a reading, or `None` for an event that
/// carries no input.
fn translate(event: EventType, pad: &mut Pad) -> Option<PadInput> {
    Some(match event {
        EventType::ButtonPressed(button, code) | EventType::ButtonReleased(button, code) => {
            let pressed = matches!(event, EventType::ButtonPressed(..));
            match classify(button) {
                Control::Button(button) => PadInput::Button { button, pressed },
                Control::Contact(dir) => {
                    pad.contacts.set(dir, pressed);
                    PadInput::Contacts(pad.contacts)
                }
                Control::Unnamed => {
                    let code = code.into_u32();
                    log::debug!(
                        "gamepad: unnamed control {code}; bind it with (button-slot N {code})"
                    );
                    PadInput::Native { code, pressed }
                }
            }
        }
        // A trigger's analog half. The digital half arrives separately as a
        // ButtonPressed and the projector holds the control while either says
        // so. Any other analog button report duplicates a digital edge that is
        // already handled.
        EventType::ButtonChanged(button, value, _) => PadInput::Trigger {
            side: trigger_side(button)?,
            value: Unit::new(value),
        },
        EventType::AxisChanged(Axis::DPadX, value, _) => {
            quantize(&mut pad.hat, value, Cardinal::Right, Cardinal::Left);
            PadInput::Hat(pad.hat)
        }
        EventType::AxisChanged(Axis::DPadY, value, _) => {
            quantize(&mut pad.hat, value, Cardinal::Up, Cardinal::Down);
            PadInput::Hat(pad.hat)
        }
        EventType::AxisChanged(axis, value, _) => {
            let side = stick_side(axis)?;
            let previous = pad.sticks[side as usize];
            let vector = match axis {
                Axis::LeftStickX | Axis::RightStickX => {
                    StickPosition::new(value, previous.y().get())
                }
                _ => StickPosition::new(previous.x().get(), value),
            };
            pad.sticks[side as usize] = vector;
            PadInput::Stick {
                side,
                value: vector,
            }
        }
        // Connection changes are handled by the caller, which needs the gilrs
        // context to describe the device. Anything else -- a dropped filter
        // event, force feedback finishing -- is not input.
        _ => return None,
    })
}

/// Collapse a driver's analog hat reading onto one axis of the set.
///
/// A hat is digital hardware, but drivers report it through an analog axis and
/// some bounce through intermediate values on the way. The band is wide
/// because only the extremes carry meaning, and a NaN reads as centered rather
/// than as a direction jammed on.
fn quantize(hat: &mut CardinalSet, value: f32, positive: Cardinal, negative: Cardinal) {
    hat.set(positive, value > 0.5);
    hat.set(negative, value < -0.5);
}

/// Everything the backend thread owns except the gilrs context itself.
///
/// Split out because building a `Gilrs` needs hardware and none of this does:
/// with the context on one side of the line, the whole event path -- latching,
/// projection, the `defsrc` gate -- can be driven from a recorded trace.
struct Dispatcher {
    engine: Arc<Mutex<PadEngine>>,
    pads: HashMap<PadDeviceId, Pad>,
    edges: Vec<PadEdge>,
    events: Vec<KeyEvent>,
}

impl Dispatcher {
    fn new(engine: Arc<Mutex<PadEngine>>) -> Dispatcher {
        Dispatcher {
            engine,
            pads: HashMap::default(),
            edges: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Handle one backend event, returning the input events it implies.
    ///
    /// `info` describes the device and is needed only for a connection, which
    /// is the one thing that has to be read out of the gilrs context.
    fn dispatch(
        &mut self,
        device: PadDeviceId,
        event: EventType,
        info: Option<PadDeviceInfo>,
    ) -> &[KeyEvent] {
        self.edges.clear();
        self.events.clear();
        // Only an analog reading or a departing controller can change what the
        // continuous projections are asking for.
        let mut resampled = matches!(event, EventType::Disconnected);
        match event {
            EventType::Connected => {
                self.pads.insert(device, Pad::default());
                if let Some(info) = info {
                    self.offer(device, info);
                }
            }
            EventType::Disconnected => {
                self.pads.remove(&device);
                let mut engine = self.engine.lock();
                if let Some(name) = engine.device_name(device) {
                    log::info!("gamepad: {name} disconnected");
                }
                engine.disconnect(device, &mut self.edges);
            }
            other => {
                let pad = self.pads.entry(device).or_default();
                if let Some(input) = translate(other, pad) {
                    // Anything that is not a digital control can change what
                    // the continuous projections are asking for -- including a
                    // d-pad contact or hat, because a d-pad reads as a stick
                    // at full deflection.
                    resampled |=
                        !matches!(input, PadInput::Button { .. } | PadInput::Native { .. });
                    // The engine lock is dropped at the end of this statement,
                    // before the gate below takes MAPPED_KEYS and before the
                    // caller sends. The processing thread takes the engine lock
                    // during a live reload, so holding it across either would
                    // let the two wait on each other.
                    self.engine.lock().feed(device, input, &mut self.edges);
                }
            }
        }
        gate_on_defsrc(&self.edges, &mut self.events);
        if resampled {
            self.wake_for_motion();
        }
        self.wake_for_pending();
        &self.events
    }

    /// Nudge the processing loop when a control starts asking for movement.
    ///
    /// A stick held at a deflection produces no edges -- it is already where
    /// the user put it -- so a mouse or scroll projection sends *nothing* down
    /// the input channel. Without this the loop would go idle, block on the
    /// channel, and leave the pointer frozen mid-push until some unrelated
    /// input happened to wake it.
    ///
    /// One nudge per transition into movement is enough: from there the tick
    /// loop keeps itself awake for as long as the demand lasts, and the next
    /// reading that returns to rest arms this again.
    fn wake_for_motion(&mut self) {
        if self.engine.lock().take_motion_start() {
            self.events
                .push(KeyEvent::new(OsCode::KEY_RESERVED, KeyValue::WakeUp));
        }
    }

    /// A debounced crossing has no edge yet, but the timer that makes it an
    /// edge belongs to the processing loop. Wake it exactly once per pending
    /// period; `is_idle` keeps the loop running until the period completes.
    fn wake_for_pending(&mut self) {
        if self.engine.lock().take_pending_start() {
            self.events
                .push(KeyEvent::new(OsCode::KEY_RESERVED, KeyValue::WakeUp));
        }
    }

    fn offer(&self, id: PadDeviceId, info: PadDeviceInfo) {
        let name = info.name.clone();
        match self.engine.lock().connect(id, info) {
            Connection::Accepted => log::info!("gamepad: using \"{name}\""),
            Connection::NotSelected => log::info!(
                "gamepad: ignoring \"{name}\", it does not match the definputdevices \
                 entry named by defgamepad"
            ),
        }
    }
}

/// Start the controller thread.
///
/// Returns immediately. Failure to reach a controller backend is logged and
/// leaves kanata running as a keyboard remapper, because a missing controller
/// library is not a reason to refuse to start.
pub fn spawn(engine: Arc<Mutex<PadEngine>>, tx: SyncSender<KeyEvent>) {
    std::thread::spawn(move || {
        if let Err(e) = run(engine, tx) {
            log::error!("gamepad: input thread stopped: {e}");
        }
    });
}

fn run(engine: Arc<Mutex<PadEngine>>, tx: SyncSender<KeyEvent>) -> Result<(), String> {
    // Default filters would apply gilrs's own deadzone and jitter handling on
    // top of ours. The projector is built to be exact, so it wants raw values.
    let mut gilrs = GilrsBuilder::new()
        .with_default_filters(false)
        .set_update_state(false)
        .build()
        .map_err(|e| format!("could not start a controller backend: {e}"))?;

    let mut dispatcher = Dispatcher::new(engine);
    for (id, pad) in gilrs.gamepads() {
        dispatcher.offer(device_id(id), describe(&pad));
    }
    log::info!(
        "gamepad: backend ready, {} controller(s) connected",
        dispatcher.engine.lock().connected_count()
    );

    loop {
        let Some(Event { id, event, .. }) = gilrs.next_event_blocking(Some(POLL_TIMEOUT)) else {
            continue;
        };
        // Reading the device out of the context is the one thing `dispatch`
        // cannot do for itself.
        let info = matches!(event, EventType::Connected).then(|| describe(&gilrs.gamepad(id)));
        for event in dispatcher.dispatch(device_id(id), event, info) {
            if tx.send(*event).is_err() {
                return Ok(());
            }
        }
    }
}

fn device_id(id: gilrs::GamepadId) -> PadDeviceId {
    PadDeviceId::new(usize::from(id) as u32)
}

fn describe(pad: &gilrs::Gamepad<'_>) -> PadDeviceInfo {
    PadDeviceInfo {
        name: pad.name().to_string(),
        vendor_id: pad.vendor_id(),
        product_id: pad.product_id(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanata_parser::gamepad::{
        Digital, Dir, DirMode, Directional, GamepadConfig, Motion, MotionKind, PadCode, PadSet,
        Projection, Unit,
    };

    fn code() -> gilrs::ev::Code {
        // gilrs exposes backend codes only through a live device, so borrow
        // one from a button that always has a mapping.
        Button::South.to_nec().expect("South always has a code")
    }

    fn axis(axis: Axis, value: f32) -> EventType {
        EventType::AxisChanged(axis, value, code())
    }

    fn translated(events: &[EventType]) -> Vec<PadInput> {
        let mut pad = Pad::default();
        events
            .iter()
            .filter_map(|event| translate(*event, &mut pad))
            .collect()
    }

    #[test]
    fn a_multi_axis_report_carries_every_axis_with_it() {
        // Backends report one axis at a time, but a projection needs the whole
        // reading: without the latch a diagonal push briefly looks cardinal.
        assert_eq!(
            translated(&[
                axis(Axis::LeftStickX, 0.5),
                axis(Axis::RightStickY, 1.0),
                axis(Axis::LeftStickY, -0.25),
            ]),
            vec![
                PadInput::Stick {
                    side: Side::Left,
                    value: StickPosition::new(0.5, 0.0)
                },
                PadInput::Stick {
                    side: Side::Right,
                    value: StickPosition::new(0.0, 1.0)
                },
                // The right stick's report must not have disturbed the left.
                PadInput::Stick {
                    side: Side::Left,
                    value: StickPosition::new(0.5, -0.25)
                },
            ]
        );
        assert_eq!(
            translated(&[axis(Axis::DPadX, 1.0), axis(Axis::DPadY, -1.0)]),
            vec![
                PadInput::Hat(CardinalSet::of(&[Cardinal::Right])),
                PadInput::Hat(CardinalSet::of(&[Cardinal::Right, Cardinal::Down])),
            ]
        );
        assert_eq!(
            translated(&[
                EventType::ButtonPressed(Button::DPadUp, code()),
                EventType::ButtonPressed(Button::DPadRight, code()),
            ]),
            vec![
                PadInput::Contacts(CardinalSet::of(&[Cardinal::Up])),
                PadInput::Contacts(CardinalSet::of(&[Cardinal::Up, Cardinal::Right])),
            ]
        );
    }

    #[test]
    fn only_the_trigger_axes_survive_as_analog_readings() {
        assert_eq!(
            translated(&[
                EventType::ButtonChanged(Button::LeftTrigger2, 0.75, code()),
                // The digital edge for this already arrived as ButtonPressed;
                // acting on the analog report too would double-fire.
                EventType::ButtonChanged(Button::South, 1.0, code()),
                EventType::ButtonChanged(Button::LeftTrigger, 1.0, code()),
                // And an axis that names no control is dropped entirely.
                axis(Axis::LeftZ, 1.0),
            ]),
            vec![PadInput::Trigger {
                side: Side::Left,
                value: Unit::new(0.75)
            }]
        );
        // A control gilrs cannot name keeps its backend code, so the user can
        // bind it to a slot.
        assert!(matches!(
            translated(&[EventType::ButtonPressed(Button::Unknown, code())]).as_slice(),
            [PadInput::Native { .. }]
        ));
    }

    #[test]
    fn a_diagonal_push_never_looks_like_a_cardinal_one() {
        // The seam between `translate` and the engine, each of which is
        // covered on its own. Both axes cross the threshold in the same frame
        // but arrive one at a time, and latching is what stops the first
        // report from leaving Right held on the way to Up-Right.
        let mut config = GamepadConfig::default();
        config.projection_mut(Directional::LeftStick).digital = Some(Digital {
            mode: DirMode::EightWay,
            ..Digital::default()
        });
        let device = PadDeviceId::new(0);
        let mut engine = PadEngine::new(config, None);
        engine.connect(device, PadDeviceInfo::default());
        let mut pad = Pad::default();
        let mut edges = Vec::new();
        for event in [axis(Axis::LeftStickX, 0.8), axis(Axis::LeftStickY, 0.8)] {
            let input = translate(event, &mut pad).expect("a reading");
            engine.feed(device, input, &mut edges);
        }
        let held = edges.iter().fold(PadSet::EMPTY, |mut set, edge| {
            set.set(edge.code, edge.pressed);
            set
        });
        assert_eq!(
            held.iter().collect::<Vec<_>>(),
            vec![PadCode::direction(Directional::LeftStick, Dir::UpRight)]
        );
    }

    // The event path the backend thread actually runs, minus the gilrs context
    // it cannot build without hardware.

    /// Drive a `Dispatcher` over a trace, with `defsrc` claiming `mapped`.
    ///
    /// `MAPPED_KEYS` is process-wide, so these serialize on the same lock the
    /// config tests use rather than racing a parse in another test.
    fn dispatch_trace(
        config: GamepadConfig,
        mapped: &[OsCode],
        trace: &[(PadDeviceId, EventType)],
    ) -> Vec<KeyEvent> {
        let _lk = match crate::tests::CFG_PARSE_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let previous = std::mem::take(&mut *crate::kanata::MAPPED_KEYS.lock());
        crate::kanata::MAPPED_KEYS
            .lock()
            .extend(mapped.iter().copied());

        let mut dispatcher = Dispatcher::new(Arc::new(Mutex::new(PadEngine::new(config, None))));
        let mut seen = Vec::new();
        for (device, event) in trace {
            let info = matches!(event, EventType::Connected).then(PadDeviceInfo::default);
            seen.extend_from_slice(dispatcher.dispatch(*device, *event, info));
        }

        *crate::kanata::MAPPED_KEYS.lock() = previous;
        seen
    }

    fn mouse_config() -> GamepadConfig {
        let mut config = GamepadConfig::default();
        *config.projection_mut(Directional::RightStick) = Projection {
            digital: None,
            motions: {
                let mut motions: kanata_parser::gamepad::Motions<Motion> = Default::default();
                motions.set(
                    MotionKind::Mouse,
                    Motion {
                        deadzone: Unit::ZERO,
                        ..Motion::MOUSE
                    },
                );
                motions
            },
        };
        config
    }

    fn push(x: f32) -> (PadDeviceId, EventType) {
        (PadDeviceId::new(0), axis(Axis::RightStickX, x))
    }

    #[test]
    fn a_stick_driving_the_pointer_wakes_the_loop_once_per_push() {
        // A stick held at a deflection produces no edges, so a mouse
        // projection sends nothing down the input channel. Without a nudge the
        // loop goes idle, blocks on that channel, and the pointer freezes
        // mid-push until some unrelated input happens to wake it. One nudge is
        // enough: from there the tick loop keeps itself awake, so a nudge per
        // sample would be channel noise at the controller's report rate.
        let mut trace = vec![(PadDeviceId::new(0), EventType::Connected)];
        trace.extend((0..10).map(|step| push(1.0 - step as f32 / 100.0)));
        let events = dispatch_trace(mouse_config(), &[], &trace);
        assert_eq!(
            events.iter().map(|e| e.value).collect::<Vec<_>>(),
            vec![KeyValue::WakeUp],
            "pushing the stick should wake the loop exactly once: {events:?}"
        );

        // Returning to rest and pushing again arms it for the next push.
        let mut trace = vec![(PadDeviceId::new(0), EventType::Connected)];
        trace.extend([push(1.0), push(0.0), push(-1.0)]);
        assert_eq!(
            dispatch_trace(mouse_config(), &[], &trace).len(),
            2,
            "the second push did not wake the loop"
        );
    }

    #[test]
    fn nothing_wakes_the_loop_when_no_control_drives_motion() {
        // The nudge exists for continuous output only. A digital config has
        // edges to send, so it may not cost a spurious wake-up.
        let mut config = GamepadConfig::default();
        config.projection_mut(Directional::RightStick).digital = Some(Digital::default());
        let mut trace = vec![
            (PadDeviceId::new(0), EventType::Connected),
            (
                PadDeviceId::new(0),
                EventType::ButtonPressed(Button::South, code()),
            ),
        ];
        trace.push(push(1.0));
        let events = dispatch_trace(
            config,
            &[PadCode::button(PadButton::South).os_code()],
            &trace,
        );
        assert!(
            events.iter().all(|e| e.value != KeyValue::WakeUp),
            "a digital config woke the loop for nothing: {events:?}"
        );
    }

    #[test]
    fn only_mapped_controls_of_a_connected_pad_reach_the_loop() {
        // The same `defsrc` gate the keyboard path uses; without it a
        // controller would press coordinates the layout does not own. A pad
        // unplugged mid-press has to release, too, or the key is stranded with
        // nothing left that could ever let go of it.
        let pad = PadDeviceId::new(3);
        let south = PadCode::button(PadButton::South).os_code();
        let events = dispatch_trace(
            GamepadConfig::default(),
            &[south],
            &[
                (pad, EventType::Connected),
                (pad, EventType::ButtonPressed(Button::South, code())),
                (pad, EventType::ButtonPressed(Button::East, code())),
                (
                    PadDeviceId::new(7),
                    EventType::ButtonPressed(Button::South, code()),
                ),
                (pad, EventType::Disconnected),
            ],
        );
        assert_eq!(
            events.iter().map(|e| (e.code, e.value)).collect::<Vec<_>>(),
            vec![(south, KeyValue::Press), (south, KeyValue::Release)]
        );
    }
}
