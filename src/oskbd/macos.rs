//! Contains the input/output code for keyboards on Macos.

#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

use super::*;
use crate::kanata::CalculatedMouseMove;
use crate::oskbd::KeyEvent;
use anyhow::anyhow;
use core_graphics::base::CGFloat;
use core_graphics::display::{CGDisplay, CGPoint};
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;
use karabiner_driverkit::*;
use libc;
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};
use os_pipe::pipe;
use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::io::Read;
use std::io::{Error, ErrorKind};
use std::os::unix::io::AsRawFd;

#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub value: u64,
    pub page: u32,
    pub code: u32,
}

impl InputEvent {
    pub fn new(event: DKEvent) -> Self {
        InputEvent {
            value: event.value,
            page: event.page,
            code: event.code,
        }
    }
}

impl From<InputEvent> for DKEvent {
    fn from(event: InputEvent) -> Self {
        Self {
            value: event.value,
            page: event.page,
            code: event.code,
        }
    }
}

pub struct KbdIn {}

impl Drop for KbdIn {
    fn drop(&mut self) {
        release();
    }
}

fn capture_stdout<F>(func: F) -> String
where
    F: FnOnce(),
{
    // Create a pipe to capture stdout
    let (mut reader, writer) = pipe().unwrap();

    // Save the original stdout file descriptor
    let stdout_fd = std::io::stdout().as_raw_fd();
    let saved_stdout = unsafe { libc::dup(stdout_fd) };

    // Redirect stdout to the pipe's writer
    unsafe {
        libc::dup2(writer.as_raw_fd(), stdout_fd);
    }

    // Close `writer` to prevent `read_to_string` deadlock: https://docs.rs/os_pipe/latest/os_pipe/#common-deadlocks-related-to-pipes
    drop(writer);

    // Run the provided function
    func();

    // Restore the original stdout
    unsafe {
        libc::dup2(saved_stdout, stdout_fd);
        libc::close(saved_stdout);
    }

    // Read all data from the pipe
    let mut captured_output = String::new();
    reader.read_to_string(&mut captured_output).unwrap();
    captured_output
}

impl KbdIn {
    pub fn new(
        include_names: Option<Vec<String>>,
        exclude_names: Option<Vec<String>>,
    ) -> Result<Self, anyhow::Error> {
        if !driver_activated() {
            return Err(anyhow!(
                "Karabiner-VirtualHIDDevice driver is not activated."
            ));
        }

        let device_names = if let Some(names) = include_names {
            validate_and_register_devices(names)
        } else if let Some(names) = exclude_names {
            // TODO: filter include_names when both exclude_names and include_names are present
            let kb_list = capture_stdout(list_keyboards);
            let names_: Vec<String> = kb_list
                .split("\n")
                .filter(|kb| !kb.is_empty() && !names.contains(&kb.to_string()))
                .map(|kb| kb.to_string())
                .collect();
            validate_and_register_devices(names_)
        } else {
            vec![]
        };

        if !device_names.is_empty() || register_device("") {
            if grab() {
                Ok(Self {})
            } else {
                Err(anyhow!("grab failed"))
            }
        } else {
            Err(anyhow!("Couldn't register any device"))
        }
    }

    pub fn read(&mut self) -> Result<InputEvent, io::Error> {
        let mut event = DKEvent {
            value: 0,
            page: 0,
            code: 0,
        };

        wait_key(&mut event);

        Ok(InputEvent::new(event))
    }
}

fn validate_and_register_devices(include_names: Vec<String>) -> Vec<String> {
    include_names
        .iter()
        .filter_map(|dev| match device_matches(dev) {
            true => Some(dev.to_string()),
            false => {
                log::warn!("Not a valid device name '{dev}'");
                None
            }
        })
        .filter_map(|dev| {
            if register_device(&dev) {
                Some(dev.to_string())
            } else {
                log::warn!("Couldn't register device '{dev}'");
                None
            }
        })
        .collect()
}

impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use kanata_keyberon::key_code::KeyCode;
        let ke = KeyEvent::try_from(*self).unwrap();
        let direction = match ke.value {
            KeyValue::Press => "↓",
            KeyValue::Release => "↑",
            KeyValue::Repeat => "⟳",
            KeyValue::Tap => "↕",
            KeyValue::WakeUp => "!",
        };
        let key_name = KeyCode::from(ke.code);
        write!(f, "{}{:?}", direction, key_name)
    }
}

impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();

    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        if let Ok(oscode) = OsCode::try_from(PageCode {
            page: item.page,
            code: item.code,
        }) {
            Ok(KeyEvent {
                code: oscode,
                value: if item.value == 1 {
                    KeyValue::Press
                } else {
                    KeyValue::Release
                },
            })
        } else {
            Err(())
        }
    }
}

impl TryFrom<KeyEvent> for InputEvent {
    type Error = ();

    fn try_from(item: KeyEvent) -> Result<Self, Self::Error> {
        if let Ok(pagecode) = PageCode::try_from(item.code) {
            let val = match item.value {
                KeyValue::Press => 1,
                _ => 0,
            };
            Ok(InputEvent {
                value: val,
                page: pagecode.page,
                code: pagecode.code,
            })
        } else {
            Err(())
        }
    }
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
pub struct KbdOut {}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        Ok(KbdOut {})
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        let mut devent = event.into();
        log::debug!("Attempting to write {event:?} {devent:?}");
        let _sent = send_key(&mut devent);
        Ok(())
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        if let Ok(event) = InputEvent::try_from(KeyEvent { value, code: key }) {
            self.write(event)
        } else {
            log::debug!("couldn't write unrecognized {key:?}");
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "OsCode not recognized!",
            ))
        }
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        if let Ok(event) = InputEvent::try_from(KeyEvent {
            value,
            code: OsCode::from_u16(code as u16).unwrap(),
        }) {
            self.write(event)
        } else {
            log::debug!("couldn't write unrecognized OsCode {code}");
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "OsCode not recognized!",
            ))
        }
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        let mut arr: [u16; 2] = [0; 2];
        c.encode_utf16(&mut arr);
        event.set_string_from_utf16_unchecked(&arr);
        event.set_type(CGEventType::KeyDown);
        event.post(CGEventTapLocation::AnnotatedSession);
        event.set_type(CGEventType::KeyUp);
        event.post(CGEventTapLocation::AnnotatedSession);
        Ok(())
    }
    pub fn scroll(&mut self, _direction: MWheelDirection, _distance: u16) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        event.set_type(CGEventType::ScrollWheel);
        match _direction {
            MWheelDirection::Down => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1,
                (_distance as i64) * 1,
            ),
            MWheelDirection::Up => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1,
                (_distance as i64) * -1,
            ),
            MWheelDirection::Left => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
                (_distance as i64) * 1,
            ),
            MWheelDirection::Right => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
                (_distance as i64) * -1,
            ),
        }
        // Mouse control only seems to work with CGEventTapLocation::HID.
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
    fn button_action(&mut self, _btn: Btn, is_click: bool) -> Result<(), io::Error> {
        let (event_type, button) = match _btn {
            Btn::Left => (
                if is_click {
                    CGEventType::LeftMouseDown
                } else {
                    CGEventType::LeftMouseUp
                },
                Some(CGMouseButton::Left),
            ),
            Btn::Right => (
                if is_click {
                    CGEventType::RightMouseDown
                } else {
                    CGEventType::RightMouseUp
                },
                Some(CGMouseButton::Right),
            ),
            Btn::Mid => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                Some(CGMouseButton::Center),
            ),
            // It's unclear to me which event type to use here, hence unsupported for now
            Btn::Forward => (CGEventType::Null, None),
            Btn::Backward => (CGEventType::Null, None),
        };
        // CGEventType doesn't implement Eq, therefore the casting to u8
        if event_type as u8 == CGEventType::Null as u8 {
            panic!("mouse buttons other than left, right, and middle aren't currently supported")
        }

        let event_source = Self::make_event_source()?;
        let event = Self::make_event()?;
        let mouse_position = event.location();
        let event =
            CGEvent::new_mouse_event(event_source, event_type, mouse_position, button.unwrap())
                .map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::Other, "Failed to create mouse event")
                })?;

        // Mouse control only seems to work with CGEventTapLocation::HID.
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    pub fn click_btn(&mut self, _btn: Btn) -> Result<(), io::Error> {
        Self::button_action(self, _btn, true)
    }

    pub fn release_btn(&mut self, _btn: Btn) -> Result<(), io::Error> {
        Self::button_action(self, _btn, false)
    }

    pub fn move_mouse(&mut self, _mv: CalculatedMouseMove) -> Result<(), io::Error> {
        let pressed = Self::pressed_buttons();

        let event_type = if pressed & 1 > 0 {
            CGEventType::LeftMouseDragged
        } else if pressed & 2 > 0 {
            CGEventType::RightMouseDragged
        } else {
            CGEventType::MouseMoved
        };

        let event = Self::make_event()?;
        let mut mouse_position = event.location();
        Self::apply_calculated_move(&_mv, &mut mouse_position);
        if let Ok(event) = CGEvent::new_mouse_event(
            Self::make_event_source()?,
            event_type,
            mouse_position,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
        }
        Ok(())
    }

    fn pressed_buttons() -> usize {
        if let Some(ns_event) = Class::get("NSEvent") {
            unsafe { msg_send![ns_event, pressedMouseButtons] }
        } else {
            0
        }
    }

    pub fn move_mouse_many(&mut self, _moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        let mut mouse_position = event.location();
        let display = CGDisplay::main();
        for current_move in _moves.iter() {
            Self::apply_calculated_move(current_move, &mut mouse_position);
        }
        display
            .move_cursor_to_point(mouse_position)
            .map_err(|_| io::Error::new(ErrorKind::Other, "failed to move mouse"))?;
        Ok(())
    }

    pub fn set_mouse(&mut self, _x: u16, _y: u16) -> Result<(), io::Error> {
        let display = CGDisplay::main();
        let point = CGPoint::new(_x as CGFloat, _y as CGFloat);
        display
            .move_cursor_to_point(point)
            .map_err(|_| io::Error::new(ErrorKind::Other, "failed to move cursor to point"))?;
        Ok(())
    }

    fn make_event_source() -> Result<CGEventSource, Error> {
        CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| {
            Error::new(
                ErrorKind::Other,
                "failed to create core graphics event source",
            )
        })
    }
    /// Creates a core graphics event.
    /// The CGEventSourceStateID is a guess at this point - all functionality works using this but
    /// I have not verified that this is the correct parameter.
    /// Note that the CFRelease function mentioned in the docs is automatically called when the
    /// event is dropped, therefore we don't need to care about this ourselves.
    fn make_event() -> Result<CGEvent, Error> {
        let event_source = Self::make_event_source()?;
        let event = CGEvent::new(event_source)
            .map_err(|_| Error::new(ErrorKind::Other, "failed to create core graphics event"))?;
        Ok(event)
    }

    /// Applies a calculated mouse move to a CGPoint.
    ///
    /// This does _not_ move the mouse, it just mutates the point.
    fn apply_calculated_move(_mv: &CalculatedMouseMove, mouse_position: &mut CGPoint) {
        match _mv.direction {
            MoveDirection::Up => mouse_position.y = mouse_position.y - _mv.distance as CGFloat,
            MoveDirection::Down => mouse_position.y = mouse_position.y + _mv.distance as CGFloat,
            MoveDirection::Left => mouse_position.x = mouse_position.x - _mv.distance as CGFloat,
            MoveDirection::Right => mouse_position.x = mouse_position.x + _mv.distance as CGFloat,
        }
    }
}
