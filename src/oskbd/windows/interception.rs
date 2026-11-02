//! Windows interception-based mechanism for reading/writing input events.

use std::io;
use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicI32, Ordering};

use kanata_interception::{
    Device, Interception, KeyState, MouseFlags, MouseState, ScanCode, Stroke,
};

use super::OsCodeWrapper;
use crate::kanata::CalculatedMouseMove;
use crate::oskbd::KeyValue;
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;

/// Key event received by the low level keyboard hook.
#[derive(Debug, Clone, Copy)]
pub struct InputEvent(pub Stroke);

use std::fmt;
impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl InputEvent {
    fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        let mut stroke = Stroke::try_from(OsCodeWrapper(code)).unwrap_or_else(|_| {
            log::error!("Trying to send unmapped oscode '{code:?}', sending esc instead");
            Stroke::Keyboard {
                code: ScanCode::Esc,
                state: KeyState::empty(),
                information: 0,
            }
        });
        match &mut stroke {
            Stroke::Keyboard { state, .. } => {
                state.set(
                    match val {
                        KeyValue::Press | KeyValue::Repeat => KeyState::DOWN,
                        KeyValue::Release => KeyState::UP,
                        KeyValue::Tap => panic!("invalid value attempted to be sent"),
                        KeyValue::WakeUp => panic!("invalid value attempted to be sent"),
                    },
                    true,
                );
            }
            _ => panic!("expected keyboard stroke"),
        }
        Self(stroke)
    }

    fn from_mouse_btn(btn: Btn, is_up: bool) -> Self {
        Self(Stroke::Mouse {
            state: match (btn, is_up) {
                (Btn::Left, true) => MouseState::LEFT_BUTTON_UP,
                (Btn::Left, false) => MouseState::LEFT_BUTTON_DOWN,
                (Btn::Right, true) => MouseState::RIGHT_BUTTON_UP,
                (Btn::Right, false) => MouseState::RIGHT_BUTTON_DOWN,
                (Btn::Mid, true) => MouseState::MIDDLE_BUTTON_UP,
                (Btn::Mid, false) => MouseState::MIDDLE_BUTTON_DOWN,
                (Btn::Backward, true) => MouseState::BUTTON_4_UP,
                (Btn::Backward, false) => MouseState::BUTTON_4_DOWN,
                (Btn::Forward, true) => MouseState::BUTTON_5_UP,
                (Btn::Forward, false) => MouseState::BUTTON_5_DOWN,
            },
            flags: MouseFlags::empty(),
            rolling: 0,
            x: 0,
            y: 0,
            information: 0,
        })
    }

    fn from_mouse_scroll(direction: MWheelDirection, distance: u16) -> Self {
        Self(Stroke::Mouse {
            state: match direction {
                MWheelDirection::Up | MWheelDirection::Down => MouseState::WHEEL,
                MWheelDirection::Left | MWheelDirection::Right => MouseState::HWHEEL,
            },
            flags: MouseFlags::empty(),
            rolling: match direction {
                MWheelDirection::Up | MWheelDirection::Right => {
                    distance.try_into().expect("checked bound of 30000 in cfg")
                }
                MWheelDirection::Down | MWheelDirection::Left => {
                    -(i16::try_from(distance).expect("checked bound of 30000 in cfg"))
                }
            },
            x: 0,
            y: 0,
            information: 0,
        })
    }

    fn from_mouse_move(direction: MoveDirection, distance: u16) -> Self {
        Self(Stroke::Mouse {
            state: MouseState::MOVE,
            flags: MouseFlags::empty(),
            rolling: 0,
            x: match direction {
                MoveDirection::Left => -i32::from(distance),
                MoveDirection::Right => i32::from(distance),
                _ => 0,
            },
            y: match direction {
                MoveDirection::Up => -i32::from(distance),
                MoveDirection::Down => i32::from(distance),
                _ => 0,
            },
            information: 0,
        })
    }

    fn from_mouse_move_many(moves: &[CalculatedMouseMove]) -> Self {
        let mut x_acc = 0;
        let mut y_acc = 0;
        for mov in moves {
            let acc_change = match mov.direction {
                MoveDirection::Up => (0, -i32::from(mov.distance)),
                MoveDirection::Down => (0, i32::from(mov.distance)),
                MoveDirection::Left => (-i32::from(mov.distance), 0),
                MoveDirection::Right => (i32::from(mov.distance), 0),
            };
            x_acc += acc_change.0;
            y_acc += acc_change.1;
        }
        Self(Stroke::Mouse {
            state: MouseState::MOVE,
            flags: MouseFlags::empty(),
            rolling: 0,
            x: x_acc,
            y: y_acc,
            information: 0,
        })
    }

    fn from_mouse_set(x: u16, y: u16) -> Self {
        Self(Stroke::Mouse {
            state: MouseState::MOVE,
            flags: MouseFlags::MOVE_ABSOLUTE | MouseFlags::VIRTUAL_DESKTOP,
            rolling: 0,
            x: i32::from(x),
            y: i32::from(y),
            information: 0,
        })
    }
}

thread_local! {
    static INTRCPTN: Interception = Interception::new().expect("interception driver should init: have you completed the interception driver installation?");
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
/// Handle for writing keys to the OS.
pub struct KbdOut {}

// Note regarding device numbers:
// Keyboard devices are 1-10 and mouse devices are 11-20. Source:
// https://github.com/oblitum/Interception/blob/39eecbbc46a52e0402f783b872ef62b0254a896a/library/interception.h#L34
const KEYBOARD_DEVICES: RangeInclusive<Device> = 1..=10;
const MOUSE_DEVICES: RangeInclusive<Device> = 11..=20;

/// Devices that keyboard and mouse output is sent to; 0 until one is known.
///
/// The driver delivers a stroke through the device it is sent to and drops it when that device
/// number has no device behind it. Which numbers are populated depends on the machine and changes
/// when devices are reconnected, so the input loop records the device of each intercepted stroke,
/// and output falls back to the first device that accepts the stroke.
pub static KEYBOARD_OUTPUT_DEVICE: AtomicI32 = AtomicI32::new(0);
pub static MOUSE_OUTPUT_DEVICE: AtomicI32 = AtomicI32::new(0);

fn write_interception(event: InputEvent) {
    let strokes = [event.0];
    log::debug!("kanata sending {:?} to driver", strokes[0]);
    let (output_device, mut devices) = match strokes[0] {
        Stroke::Keyboard { .. } => (&KEYBOARD_OUTPUT_DEVICE, KEYBOARD_DEVICES),
        Stroke::Mouse { .. } => (&MOUSE_OUTPUT_DEVICE, MOUSE_DEVICES),
    };
    INTRCPTN.with(|ic| {
        // send returns the number of strokes the driver accepted: 0 means nothing was delivered.
        let dev = output_device.load(Ordering::Relaxed);
        if dev != 0 && ic.send(dev, &strokes) == 1 {
            return;
        }
        match devices.find(|&d| d != dev && ic.send(d, &strokes) == 1) {
            Some(d) => {
                log::info!("sending output to interception device {d}");
                output_device.store(d, Ordering::Relaxed);
            }
            None => log::error!("no interception device accepted {:?}", strokes[0]),
        }
    })
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {})
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        write_interception(event);
        Ok(())
    }

    pub fn write_code_raw(&mut self, code: u16, value: KeyValue) -> Result<(), io::Error> {
        super::write_code_raw(code, value)
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        super::write_code(code as u16, value)
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        self.write(InputEvent::from_oscode(key, value))
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        log::debug!("click btn: {:?}", btn);
        write_interception(InputEvent::from_mouse_btn(btn, false));
        Ok(())
    }

    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        log::debug!("release btn: {:?}", btn);
        let event = InputEvent::from_mouse_btn(btn, true);
        write_interception(event);
        Ok(())
    }

    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        log::debug!("scroll: {direction:?} {distance:?}");
        write_interception(InputEvent::from_mouse_scroll(direction, distance));
        Ok(())
    }

    /// Send using VK_PACKET
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        super::send_uc(c, false);
        super::send_uc(c, true);
        Ok(())
    }

    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        write_interception(InputEvent::from_mouse_move(mv.direction, mv.distance));
        Ok(())
    }

    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        write_interception(InputEvent::from_mouse_move_many(moves));
        Ok(())
    }

    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        write_interception(InputEvent::from_mouse_set(x, y));
        Ok(())
    }
}
