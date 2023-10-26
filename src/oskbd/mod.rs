//! Platform specific code for low level keyboard read/write.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

// ------------------ KeyValue --------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyValue {
    Release = 0,
    Press = 1,
    Repeat = 2,
    Tap,
}

impl From<i32> for KeyValue {
    fn from(item: i32) -> Self {
        match item {
            0 => Self::Release,
            1 => Self::Press,
            2 => Self::Repeat,
            _ => unreachable!(),
        }
    }
}

impl From<bool> for KeyValue {
    fn from(up: bool) -> Self {
        match up {
            true => Self::Release,
            false => Self::Press,
        }
    }
}

impl From<KeyValue> for bool {
    fn from(val: KeyValue) -> Self {
        matches!(val, KeyValue::Release)
    }
}

use kanata_parser::{custom_action::MWheelDirection, keys::OsCode};

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub code: OsCode,
    pub value: KeyValue,
}

#[cfg(not(all(feature = "interception_driver", target_os = "windows")))]
impl KeyEvent {
    pub fn new(code: OsCode, value: KeyValue) -> Self {
        Self { code, value }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ScrollEventKind {
    Standard,
    HiRes,
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollEvent {
    pub kind: ScrollEventKind,
    pub direction: MWheelDirection,
    /// Unit: scroll notches if ScrollEventKind::Standard or
    /// scroll notches * 120 if ScrollEventKind::HiRes
    pub distance: u32,
}

impl TryFrom<ScrollEvent> for OsCode {
    type Error = ();
    fn try_from(value: ScrollEvent) -> Result<Self, Self::Error> {
        match value.kind {
            ScrollEventKind::Standard => Ok(match value.direction {
                MWheelDirection::Up => OsCode::MouseWheelUp,
                MWheelDirection::Down => OsCode::MouseWheelDown,
                MWheelDirection::Left => OsCode::MouseWheelLeft,
                MWheelDirection::Right => OsCode::MouseWheelRight,
            }),
            ScrollEventKind::HiRes => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SupportedInputEvent {
    KeyEvent(KeyEvent),
    ScrollEvent(ScrollEvent),
}

impl TryFrom<SupportedInputEvent> for KeyEvent {
    type Error = ();
    fn try_from(value: SupportedInputEvent) -> Result<Self, Self::Error> {
        Ok(match value {
            SupportedInputEvent::KeyEvent(kev) => kev,
            SupportedInputEvent::ScrollEvent(sev) => KeyEvent {
                code: sev.try_into()?,
                value: KeyValue::Tap,
            },
        })
    }
}
