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

use kanata_parser::{custom_action::MoveDirection, keys::OsCode};

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
    pub direction: MoveDirection,
    /// Unit: scroll notches if ScrollEventKind::Standard or
    /// scroll notches * 120 if ScrollEventKind::HiRes
    pub distance: u32,
}

impl TryFrom<ScrollEvent> for OsCode {
    type Error = ();
    fn try_from(value: ScrollEvent) -> Result<Self, Self::Error> {
        match value.kind {
            ScrollEventKind::Standard => {
                Ok(match value.direction {
                    MoveDirection::Up => OsCode::MouseWheelUp,
                    MoveDirection::Down => OsCode::MouseWheelDown,
                    MoveDirection::Left => OsCode::MouseWheelLeft,
                    MoveDirection::Right => OsCode::MouseWheelRight,
                })
            }
            ScrollEventKind::HiRes => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SupportedInputEvent {
    KeyEvent(KeyEvent),
    ScrollEvent(ScrollEvent),
}
