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
