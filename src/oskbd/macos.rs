//! Contains the input/output code for keyboards on Macos.

// Caused by unmaintained objc crate triggering warnings.
#![allow(unexpected_cfgs)]
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
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::io::Error;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub value: u64,
    pub page: u32,
    pub code: u32,
    pub device_hash: u64,
}

impl InputEvent {
    pub fn new(event: DKEvent) -> Self {
        InputEvent {
            value: event.value,
            page: event.page,
            code: event.code,
            device_hash: event.device_hash,
        }
    }
}

impl From<InputEvent> for DKEvent {
    fn from(event: InputEvent) -> Self {
        Self {
            value: event.value,
            page: event.page,
            code: event.code,
            // Output events don't originate from a physical device.
            device_hash: 0,
        }
    }
}

pub struct KbdIn {
    grabbed: bool,
    device_hash_to_id: HashMap<u64, std::num::NonZeroU8>,
}

impl Drop for KbdIn {
    fn drop(&mut self) {
        if self.grabbed {
            release();
        }
    }
}

impl KbdIn {
    pub fn new(
        include_names: Option<Vec<String>>,
        exclude_names: Option<Vec<String>>,
        input_devices: Option<&[(std::num::NonZeroU8, kanata_parser::cfg::InputDeviceMatcher)]>,
    ) -> Result<Self, anyhow::Error> {
        if !driver_activated() {
            return Err(anyhow!(
                "Karabiner-VirtualHIDDevice driver is not activated."
            ));
        }

        // Based on the definition of include and exclude names, they should never be used together.
        // Kanata config parser should probably enforce this.
        let has_device_filter = include_names.is_some() || exclude_names.is_some();
        let device_names = if let Some(included_names) = include_names {
            validate_and_register_devices(included_names)
        } else if let Some(excluded_names) = exclude_names {
            // get all devices
            let kb_list = fetch_devices();

            // filter out excluded devices
            let devices_to_include = kb_list
                .iter()
                .filter(|k| !excluded_names.iter().any(|n| *k == n.as_str()))
                .map(|k| {
                    if k.product_key.trim().is_empty() {
                        format!("{:x}", k.hash)
                    } else {
                        k.product_key.clone()
                    }
                })
                .collect::<Vec<String>>();

            // register the remeining devices
            validate_and_register_devices(devices_to_include)
        } else {
            vec![]
        };

        // When an include/exclude list is configured but no devices matched,
        // do NOT fall back to registering all devices. Only use the catch-all
        // register_device("") when no device filter was specified at all.
        if !device_names.is_empty() || (!has_device_filter && register_device("")) {
            if grab() {
                let device_hash_to_id = build_device_hash_to_id_map(input_devices);
                Ok(Self {
                    grabbed: true,
                    device_hash_to_id,
                })
            } else {
                Err(anyhow!("grab failed"))
            }
        } else {
            Err(anyhow!(
                "Couldn't register any device. Use 'kanata --list' to see available devices. \
                 Note: devices with empty names are automatically skipped to prevent crashes."
            ))
        }
    }

    pub fn read(&mut self) -> Result<InputEvent, io::Error> {
        let mut event = DKEvent {
            value: 0,
            page: 0,
            code: 0,
            device_hash: 0,
        };

        let got_event = wait_key(&mut event);
        if got_event == 0 {
            // Pipe returned EOF — input was released via release_input_only()
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "input pipe closed (devices released)",
            ));
        }

        Ok(InputEvent::new(event))
    }

    /// Look up the device ID for an event's device hash.
    pub fn device_id_for_hash(&self, hash: u64) -> Option<std::num::NonZeroU8> {
        self.device_hash_to_id.get(&hash).copied()
    }

    /// Release seized input devices without tearing down the output connection.
    /// After this call, `read()` will return `UnexpectedEof`.
    pub fn release_input(&mut self) {
        if self.grabbed {
            release_input_only();
            self.grabbed = false;
        }
    }

    /// Re-seize input devices after a previous `release_input()`.
    /// Returns true if at least one device was seized.
    pub fn regrab_input(&mut self) -> bool {
        if !self.grabbed {
            let ok = karabiner_driverkit::regrab_input();
            self.grabbed = ok;
            ok
        } else {
            true
        }
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }
}

/// Build a mapping from device hashes to configured device IDs by matching
/// the connected devices against the `definputdevices` matchers.
fn build_device_hash_to_id_map(
    input_devices: Option<&[(std::num::NonZeroU8, kanata_parser::cfg::InputDeviceMatcher)]>,
) -> HashMap<u64, std::num::NonZeroU8> {
    use std::num::NonZeroU8;
    let mut map: HashMap<u64, NonZeroU8> = HashMap::new();
    let Some(matchers) = input_devices else {
        return map;
    };
    let kb_list = fetch_devices();
    for (id, matcher) in matchers.iter() {
        for kb in &kb_list {
            let name_matches = matcher
                .name
                .as_ref()
                .is_none_or(|n| kb.product_key.contains(n.as_str()));
            let hash_matches = matcher.hash.as_ref().is_none_or(|h| {
                let device_hash = format!("{:x}", kb.hash);
                device_hash.eq_ignore_ascii_case(h)
            });
            let vendor_matches = matcher
                .vendor_id
                .is_none_or(|v| u16::try_from(kb.vendor_id) == Ok(v));
            let product_matches = matcher
                .product_id
                .is_none_or(|p| u16::try_from(kb.product_id) == Ok(p));
            if name_matches && hash_matches && vendor_matches && product_matches {
                if let Some(existing_id) = map.get(&kb.hash) {
                    log::warn!(
                        "definputdevices: device \"{}\" (hash {:x}) also matches ID {id}, \
                         keeping first match ID {existing_id}",
                        kb.product_key,
                        kb.hash
                    );
                    continue;
                }
                log::info!(
                    "definputdevices: device ID {id} matched \"{}\" (hash {:x})",
                    kb.product_key,
                    kb.hash
                );
                map.insert(kb.hash, *id);
            }
        }
    }
    map
}

fn validate_and_register_devices(include_names: Vec<String>) -> Vec<String> {
    include_names
        .iter()
        .filter_map(|dev| {
            // Defensive check: skip empty device names that could cause crashes
            if dev.trim().is_empty() {
                log::warn!("Skipping empty device name (likely old keyboard without proper identification)");
                return None;
            }

            // Also skip the Karabiner device
            // driverkit already prevents registering it, but this avoids unnecessary warnings
            if dev.to_lowercase().contains("karabiner") {
                return None;
            }

            match device_matches(dev) {
                true => Some(dev.to_string()),
                false => {
                    log::warn!("'{dev}' doesn't match any connected device");
                    None
                }
            }
        })
        .filter_map(|dev| {
            if register_device(&dev) {
                Some(dev.to_string())
            } else {
                log::warn!("Couldn't register device '{}' - device may be in use by another application or disconnected", dev);
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
        write!(f, "{direction}{key_name:?}")
    }
}

impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();

    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        if let Ok(oscode) = OsCode::try_from(PageCode {
            page: item.page,
            code: item.code,
        }) {
            Ok(KeyEvent::new(
                oscode,
                if item.value == 1 {
                    KeyValue::Press
                } else {
                    KeyValue::Release
                },
            ))
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
                KeyValue::Press | KeyValue::Repeat => 1,
                _ => 0,
            };
            Ok(InputEvent {
                value: val,
                page: pagecode.page,
                code: pagecode.code,
                device_hash: 0,
            })
        } else {
            Err(())
        }
    }
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
pub struct KbdOut {
    output_pressed_since: HashMap<OsCode, Instant>,
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        Ok(KbdOut {
            output_pressed_since: HashMap::default(),
        })
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        let mut devent = event.into();
        log::debug!("Attempting to write {event:?} {devent:?}");
        let rc = send_key(&mut devent);
        if rc == 2 {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "DriverKit virtual keyboard not ready (sink disconnected)",
            ));
        }
        Ok(())
    }

    pub fn output_ready(&self) -> bool {
        is_sink_ready()
    }

    pub fn wait_until_ready(&self, timeout: Option<Duration>) -> bool {
        let start = Instant::now();
        let mut attempt = 0u32;

        loop {
            if self.output_ready() {
                return true;
            }

            if let Some(timeout) = timeout
                && start.elapsed() >= timeout
            {
                return false;
            }

            attempt += 1;
            if attempt % 10 == 0 {
                if let Some(timeout) = timeout {
                    log::info!(
                        "Waiting for DriverKit virtual keyboard... ({:.1}s/{:.1}s)",
                        start.elapsed().as_secs_f64(),
                        timeout.as_secs_f64()
                    );
                } else {
                    log::info!(
                        "Waiting for DriverKit virtual keyboard... ({:.1}s)",
                        start.elapsed().as_secs_f64()
                    );
                }
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        if let Ok(event) = InputEvent::try_from(KeyEvent::new(key, value)) {
            let result = self.write(event);
            if result.is_ok() {
                self.record_output_transition_after_write(key, value);
            }
            result
        } else {
            log::debug!("couldn't write unrecognized {key:?}");
            Err(io::Error::other("OsCode not recognized!"))
        }
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        if let Ok(event) =
            InputEvent::try_from(KeyEvent::new(OsCode::from_u16(code as u16).unwrap(), value))
        {
            self.write(event)
        } else {
            log::debug!("couldn't write unrecognized OsCode {code}");
            Err(io::Error::other("OsCode not recognized!"))
        }
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    pub fn release_tracked_output_keys(&mut self, reason: &str) {
        let tracked_keys: Vec<OsCode> = self.output_pressed_since.keys().copied().collect();
        if tracked_keys.is_empty() {
            return;
        }

        for key in tracked_keys {
            if let Err(error) = self.write_key(key, KeyValue::Release) {
                log::warn!(
                    "failed to release tracked output key during {} recovery: key={key:?} error={error}",
                    reason
                );
            }
        }

        self.output_pressed_since.clear();
    }

    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        let mut arr = [0u16; 2];
        // Capture the slice containing the encoded UTF-16 code units.
        let encoded = c.encode_utf16(&mut arr);
        // Pass only the part of the array that was populated.
        event.set_string_from_utf16_unchecked(encoded);
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
                _distance as i64,
            ),
            MWheelDirection::Up => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1,
                -(_distance as i64),
            ),
            MWheelDirection::Left => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
                _distance as i64,
            ),
            MWheelDirection::Right => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
                -(_distance as i64),
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
                .map_err(|_| std::io::Error::other("Failed to create mouse event"))?;

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
            .map_err(|_| io::Error::other("failed to move mouse"))?;
        Ok(())
    }

    pub fn set_mouse(&mut self, _x: u16, _y: u16) -> Result<(), io::Error> {
        let display = CGDisplay::main();
        let point = CGPoint::new(_x as CGFloat, _y as CGFloat);
        display
            .move_cursor_to_point(point)
            .map_err(|_| io::Error::other("failed to move cursor to point"))?;
        Ok(())
    }

    fn make_event_source() -> Result<CGEventSource, Error> {
        CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| Error::other("failed to create core graphics event source"))
    }
    /// Creates a core graphics event.
    /// The CGEventSourceStateID is a guess at this point - all functionality works using this but
    /// I have not verified that this is the correct parameter.
    /// Note that the CFRelease function mentioned in the docs is automatically called when the
    /// event is dropped, therefore we don't need to care about this ourselves.
    fn make_event() -> Result<CGEvent, Error> {
        let event_source = Self::make_event_source()?;
        let event = CGEvent::new(event_source)
            .map_err(|_| Error::other("failed to create core graphics event"))?;
        Ok(event)
    }

    fn record_output_transition_after_write(&mut self, key: OsCode, value: KeyValue) {
        match value {
            KeyValue::Press | KeyValue::Repeat => {
                self.output_pressed_since
                    .entry(key)
                    .or_insert_with(Instant::now);
            }
            KeyValue::Release => {
                self.output_pressed_since.remove(&key);
            }
            KeyValue::Tap | KeyValue::WakeUp => {}
        }
    }

    /// Applies a calculated mouse move to a CGPoint.
    ///
    /// This does _not_ move the mouse, it just mutates the point.
    fn apply_calculated_move(_mv: &CalculatedMouseMove, mouse_position: &mut CGPoint) {
        match _mv.direction {
            MoveDirection::Up => mouse_position.y -= _mv.distance as CGFloat,
            MoveDirection::Down => mouse_position.y += _mv.distance as CGFloat,
            MoveDirection::Left => mouse_position.x -= _mv.distance as CGFloat,
            MoveDirection::Right => mouse_position.x += _mv.distance as CGFloat,
        }
    }
}
