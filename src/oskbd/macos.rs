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
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_graphics::base::CGFloat;
use core_graphics::display::{CGDisplay, CGPoint};
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CGMouseButton, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use kanata_parser::cfg::MappedKeys;
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
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender as Sender;
use std::time::{Duration, Instant};

/// Mouse `OsCode`s that, when present in `MAPPED_KEYS`, justify installing the
/// CGEventTap. Used both as the startup/reload install gate and as the set of
/// codes the tap can produce.
const MOUSE_OSCODES: [OsCode; 9] = [
    OsCode::BTN_LEFT,
    OsCode::BTN_RIGHT,
    OsCode::BTN_MIDDLE,
    OsCode::BTN_SIDE,
    OsCode::BTN_EXTRA,
    OsCode::MouseWheelUp,
    OsCode::MouseWheelDown,
    OsCode::MouseWheelLeft,
    OsCode::MouseWheelRight,
];

/// Sender stashed by the first `start_mouse_listener` call so that
/// `ensure_mouse_listener_installed_after_reload` can install the tap on a
/// later live reload without needing the original `event_loop` context.
static MOUSE_TAP_TX: OnceLock<Sender<KeyEvent>> = OnceLock::new();

/// Tracks whether `start_mouse_listener` has *claimed* the install slot —
/// i.e. promised to spawn a thread that will create and enable a CGEventTap.
/// Claimed via `compare_exchange` *before* `thread::spawn` so a concurrent
/// live reload cannot race in and install a second tap during the brief
/// window before the spawned thread reaches `tap.enable()`. Reset to `false`
/// if `CGEventTap::new` fails, so a future reload (e.g. after the user grants
/// Accessibility permission) can retry.
///
/// Note that "claimed" is slightly stronger than "currently capturing
/// events": there is a sub-millisecond gap between the claim and
/// `tap.enable()` during which no events flow yet. Reload callers
/// short-circuit in that gap, which is correct because the spawned thread
/// will deliver the working tap regardless.
static MOUSE_TAP_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Stashed by the first `start_mouse_listener` call so the CGEventTap callback
/// can read the live `mouse-movement-key` setting on every cursor movement
/// event. The Arc points to the same `parking_lot::Mutex` that the live-reload
/// path updates, so changes take effect with no extra plumbing.
static MOUSE_MOVEMENT_KEY: OnceLock<std::sync::Arc<parking_lot::Mutex<Option<OsCode>>>> =
    OnceLock::new();

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
            device_hash: 0,
        }
    }
}

pub struct KbdIn {
    grabbed: bool,
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
                Ok(Self { grabbed: true })
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
                KeyValue::Press | KeyValue::Repeat => 1,
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
pub struct KbdOut {
    output_pressed_since: HashMap<OsCode, Instant>,
}

/// Treat a sink-disconnect from the processing thread as a non-fatal drop.
///
/// The macOS event loop (`src/kanata/macos.rs`) coordinates recovery when the
/// DriverKit sink goes away (e.g. on wake-from-sleep) by polling
/// `output_ready()` and re-grabbing input. The processing thread runs in
/// parallel and can race ahead, attempting a write before the event loop
/// notices. Without this, that write would propagate `NotConnected` up to
/// `handle_keys` and panic the processing loop.
#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
fn drop_if_sink_disconnected(
    err: io::Error,
    key: OsCode,
    value: KeyValue,
) -> Result<(), io::Error> {
    if err.kind() == io::ErrorKind::NotConnected {
        log::warn!("dropping {key:?} {value:?}: output backend unavailable (will recover)");
        Ok(())
    } else {
        Err(err)
    }
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
        if let Ok(event) = InputEvent::try_from(KeyEvent { value, code: key }) {
            match self.write(event) {
                Ok(()) => {
                    self.record_output_transition_after_write(key, value);
                    Ok(())
                }
                Err(e) => drop_if_sink_disconnected(e, key, value),
            }
        } else {
            log::debug!("couldn't write unrecognized {key:?}");
            Err(io::Error::other("OsCode not recognized!"))
        }
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        let key = OsCode::from_u16(code as u16).unwrap();
        if let Ok(event) = InputEvent::try_from(KeyEvent { value, code: key }) {
            match self.write(event) {
                Ok(()) => Ok(()),
                Err(e) => drop_if_sink_disconnected(e, key, value),
            }
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
    /// Synthesize a mouse button press or release via CGEvent.
    ///
    /// Side buttons (Backward/Forward) use OtherMouseDown/Up with
    /// CGMouseButton::Center as a placeholder, then override the
    /// MOUSE_EVENT_BUTTON_NUMBER field to the real index (3=Back, 4=Forward).
    /// The Rust CGMouseButton enum only has 3 variants but the underlying
    /// Apple API supports up to 32 buttons via this field.
    ///
    /// Ref: [init(mouseEventSource:mouseType:mouseCursorPosition:mouseButton:)][1], [setIntegerValueField][2]
    ///
    /// [1]: https://developer.apple.com/documentation/coregraphics/cgevent/init(mouseeventsource:mousetype:mousecursorposition:mousebutton:)
    /// [2]: https://developer.apple.com/documentation/coregraphics/cgevent/setintegervaluefield(_:value:)
    fn button_action(&mut self, _btn: Btn, is_click: bool) -> Result<(), io::Error> {
        // (event_type, placeholder_button, real_button_number_override)
        let (event_type, button, button_number) = match _btn {
            Btn::Left => (
                if is_click {
                    CGEventType::LeftMouseDown
                } else {
                    CGEventType::LeftMouseUp
                },
                CGMouseButton::Left,
                None,
            ),
            Btn::Right => (
                if is_click {
                    CGEventType::RightMouseDown
                } else {
                    CGEventType::RightMouseUp
                },
                CGMouseButton::Right,
                None,
            ),
            Btn::Mid => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                CGMouseButton::Center,
                None,
            ),
            // Side buttons use OtherMouseDown/Up (same event type as middle click)
            // with the button number overridden after event creation.
            Btn::Backward => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                CGMouseButton::Center,
                Some(3), // USB HID button 4 -> CGEvent button 3 (0-indexed)
            ),
            Btn::Forward => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                CGMouseButton::Center,
                Some(4), // USB HID button 5 -> CGEvent button 4 (0-indexed)
            ),
        };

        let event_source = Self::make_event_source()?;
        let event = Self::make_event()?;
        let mouse_position = event.location();
        let event = CGEvent::new_mouse_event(event_source, event_type, mouse_position, button)
            .map_err(|_| std::io::Error::other("Failed to create mouse event"))?;

        if let Some(num) = button_number {
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, num);
        }

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

/// Convert a `(CGEventType, button_number)` pair from a CGEventTap into a
/// kanata `KeyEvent`. The button number field is only meaningful for
/// `OtherMouseDown`/`OtherMouseUp` (2=Middle, 3=Back, 4=Forward); Left/Right
/// are determined entirely by the event type.
impl TryFrom<(CGEventType, i64)> for KeyEvent {
    type Error = ();
    fn try_from((event_type, button_number): (CGEventType, i64)) -> Result<Self, ()> {
        use OsCode::*;
        let (code, value) = match event_type {
            CGEventType::LeftMouseDown => (BTN_LEFT, KeyValue::Press),
            CGEventType::LeftMouseUp => (BTN_LEFT, KeyValue::Release),
            CGEventType::RightMouseDown => (BTN_RIGHT, KeyValue::Press),
            CGEventType::RightMouseUp => (BTN_RIGHT, KeyValue::Release),
            CGEventType::OtherMouseDown | CGEventType::OtherMouseUp => {
                let code = match button_number {
                    2 => BTN_MIDDLE,
                    3 => BTN_SIDE,
                    4 => BTN_EXTRA,
                    _ => return Err(()),
                };
                let value = if matches!(event_type, CGEventType::OtherMouseDown) {
                    KeyValue::Press
                } else {
                    KeyValue::Release
                };
                (code, value)
            }
            _ => return Err(()),
        };
        Ok(KeyEvent { code, value })
    }
}

/// Decode a `ScrollWheel` `CGEvent` into a kanata `KeyEvent`. A scroll event
/// may carry both axes simultaneously (diagonal scroll on a trackpad); we
/// pick the dominant axis with vertical winning ties, matching how Linux
/// processes one `REL_WHEEL`/`REL_HWHEEL` at a time. The axis/sign convention
/// mirrors `OsKbdOut::scroll`.
fn scroll_event_to_key_event(event: &CGEvent) -> Option<KeyEvent> {
    use OsCode::*;
    let dy = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
    let dx = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
    let code = if dy.abs() >= dx.abs() {
        match dy.signum() {
            1 => MouseWheelDown,
            -1 => MouseWheelUp,
            _ => return None,
        }
    } else {
        match dx.signum() {
            1 => MouseWheelLeft,
            -1 => MouseWheelRight,
            _ => return None,
        }
    };
    Some(KeyEvent {
        code,
        value: KeyValue::Tap,
    })
}

/// Start a CGEventTap on a background thread to intercept mouse button events
/// and (optionally) cursor movement events. macOS equivalent of the Windows
/// mouse hook in `windows/llhook.rs` plus the cursor-movement branch of the
/// Linux event loop.
///
/// Mapped buttons are suppressed and forwarded to the processing channel;
/// unmapped buttons pass through. If `mouse_movement_key` is `Some`, every
/// cursor movement (including drags) sends a synthetic `Tap` of the configured
/// `OsCode` on the channel without suppressing the underlying movement event.
///
/// Only installed if the config has mouse buttons in defsrc OR
/// `mouse-movement-key` is configured.
///
/// Requires Accessibility or Input Monitoring permission.
pub fn start_mouse_listener(
    tx: Sender<KeyEvent>,
    mapped_keys: &MappedKeys,
    mouse_movement_key: std::sync::Arc<parking_lot::Mutex<Option<OsCode>>>,
) -> Option<std::thread::JoinHandle<()>> {
    // Stash both unconditionally so the reload helper always has them, even
    // if this initial call bails on the install gate. `OnceLock::set` is a
    // no-op on subsequent calls — we rely on the single-process,
    // single-Kanata assumption: the inner `parking_lot::Mutex` is shared with
    // `do_live_reload`, so reloads mutate the *value*, never replace the
    // Arc. The `debug_assert!` surfaces accidental violations in test builds.
    let tx_was_unset = MOUSE_TAP_TX.set(tx.clone()).is_ok();
    let _ = MOUSE_MOVEMENT_KEY.set(mouse_movement_key.clone());
    debug_assert!(
        tx_was_unset
            || std::sync::Arc::ptr_eq(
                MOUSE_MOVEMENT_KEY
                    .get()
                    .expect("set above or already present"),
                &mouse_movement_key,
            ),
        "start_mouse_listener called twice with a different mouse_movement_key Arc — \
         the previously stashed Arc would be silently kept"
    );

    let has_mouse_keys = MOUSE_OSCODES.iter().any(|c| mapped_keys.contains(c));
    let has_movement_key = mouse_movement_key.lock().is_some();
    if !has_mouse_keys && !has_movement_key {
        log::info!(
            "No mouse buttons/wheel in defsrc and no mouse-movement-key configured. \
             Not installing mouse event tap."
        );
        return None;
    }

    // Claim the install slot atomically *before* spawning. Closes the race
    // where a live reload could observe `MOUSE_TAP_INSTALLED == false` between
    // the spawn here and the spawned thread's `tap.enable()`, and try to
    // install a second tap. If the claim fails, an installation is already in
    // progress (or completed) — the running tap reads both globals live, so
    // this caller has nothing to do.
    if MOUSE_TAP_INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }

    let handle = std::thread::Builder::new()
        .name("mouse-event-tap".into())
        .spawn(move || {
            let events_of_interest = vec![
                CGEventType::LeftMouseDown,
                CGEventType::LeftMouseUp,
                CGEventType::RightMouseDown,
                CGEventType::RightMouseUp,
                CGEventType::OtherMouseDown,
                CGEventType::OtherMouseUp,
                CGEventType::ScrollWheel,
                CGEventType::MouseMoved,
                CGEventType::LeftMouseDragged,
                CGEventType::RightMouseDragged,
                CGEventType::OtherMouseDragged,
            ];

            let tap = match CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                events_of_interest,
                // Callback receives &CGEvent; return Some(clone) to pass through,
                // None to suppress the event.
                move |_proxy, event_type, event| {
                    // Cursor movement (incl. drags while a button is held).
                    // Always pass through — never suppress, or the cursor freezes.
                    if matches!(
                        event_type,
                        CGEventType::MouseMoved
                            | CGEventType::LeftMouseDragged
                            | CGEventType::RightMouseDragged
                            | CGEventType::OtherMouseDragged
                    ) {
                        // The Arc is stashed before this tap is created, so
                        // `get()` is `Some` in practice. Fall back to a plain
                        // pass-through if not, rather than panicking on the
                        // hot path.
                        let mmk_slot = match MOUSE_MOVEMENT_KEY.get() {
                            Some(slot) => slot,
                            None => return Some(event.clone()),
                        };
                        if let Some(code) = *mmk_slot.lock() {
                            let fake = KeyEvent {
                                code,
                                value: KeyValue::Tap,
                            };
                            if let Err(e) = tx.try_send(fake) {
                                // Drops are expected under high movement rates;
                                // the user only needs one tap to refresh their
                                // hold timer, so this is not user-visible.
                                log::trace!("mouse tap (movement): drop synthetic tap: {e}");
                            }
                        }
                        return Some(event.clone());
                    }

                    if matches!(event_type, CGEventType::ScrollWheel) {
                        let Some(key_event) = scroll_event_to_key_event(event) else {
                            return Some(event.clone());
                        };
                        if !crate::kanata::MAPPED_KEYS.lock().contains(&key_event.code) {
                            return Some(event.clone());
                        }
                        log::debug!("mouse tap (wheel): {key_event:?}");
                        if let Err(e) = tx.try_send(key_event) {
                            log::warn!("mouse tap: failed to send wheel event: {e}");
                            return Some(event.clone());
                        }
                        return None;
                    }

                    let button_number =
                        event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER);
                    let mut key_event = match KeyEvent::try_from((event_type, button_number)) {
                        Ok(ev) => ev,
                        Err(()) => return Some(event.clone()),
                    };

                    if !crate::kanata::MAPPED_KEYS.lock().contains(&key_event.code) {
                        return Some(event.clone());
                    }

                    // Track pressed state to convert duplicate presses into repeats,
                    // matching the keyboard event loop behavior.
                    match key_event.value {
                        KeyValue::Release => {
                            crate::kanata::PRESSED_KEYS.lock().remove(&key_event.code);
                        }
                        KeyValue::Press => {
                            let mut pressed_keys = crate::kanata::PRESSED_KEYS.lock();
                            if pressed_keys.contains(&key_event.code) {
                                key_event.value = KeyValue::Repeat;
                            } else {
                                pressed_keys.insert(key_event.code);
                            }
                        }
                        _ => {}
                    }

                    log::debug!("mouse tap: {key_event:?}");

                    if let Err(e) = tx.try_send(key_event) {
                        log::warn!("mouse tap: failed to send event: {e}");
                        return Some(event.clone());
                    }

                    // Suppress the original event so it doesn't reach the system.
                    None
                },
            ) {
                Ok(tap) => tap,
                Err(()) => {
                    log::error!(
                        "Failed to create mouse event tap. \
                         Ensure kanata has Accessibility or Input Monitoring permission \
                         in System Settings > Privacy & Security."
                    );
                    // Release the install claim so a future live reload can
                    // retry once the user grants permission.
                    MOUSE_TAP_INSTALLED.store(false, Ordering::Release);
                    return;
                }
            };

            let loop_source = tap
                .mach_port
                .create_runloop_source(0)
                .expect("failed to create CFRunLoop source for mouse event tap");
            // Safety: kCFRunLoopCommonModes is an extern static from CoreFoundation.
            // Accessing it requires unsafe but is always valid in a running process.
            let mode = unsafe { kCFRunLoopCommonModes };
            CFRunLoop::get_current().add_source(&loop_source, mode);
            tap.enable();
            // MOUSE_TAP_INSTALLED was already set by the caller via
            // compare_exchange before this thread was spawned.
            log::info!("Mouse event tap installed and active.");
            CFRunLoop::run_current();
        })
        .expect("failed to spawn mouse event tap thread");

    Some(handle)
}

/// Re-attempt installing the mouse event tap after a live reload. The running
/// tap callback already reads `MAPPED_KEYS` and `MOUSE_MOVEMENT_KEY` live, so
/// if the tap is already up there is nothing to do — but if a reload introduces
/// the first mouse key in defsrc or the first `mouse-movement-key` value, the
/// startup-time install gate may have skipped installation, and we need to
/// install now.
pub fn ensure_mouse_listener_installed_after_reload() {
    if MOUSE_TAP_INSTALLED.load(Ordering::Acquire) {
        // Existing tap reads both MAPPED_KEYS and MOUSE_MOVEMENT_KEY live.
        return;
    }
    let Some(tx) = MOUSE_TAP_TX.get().cloned() else {
        log::debug!("mouse tap reload hook: no tx stashed yet, skipping");
        return;
    };
    let Some(mmk) = MOUSE_MOVEMENT_KEY.get().cloned() else {
        log::debug!("mouse tap reload hook: no mouse_movement_key stashed yet, skipping");
        return;
    };
    let mapped = crate::kanata::MAPPED_KEYS.lock();
    let _ = start_mouse_listener(tx, &mapped, mmk);
}
