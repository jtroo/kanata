//! Redirects output to the function provided by the entity supplying simulated input (e.g., AHK)
// todo: allow sharing numpad status to differentiate between vk enter and vk numpad enter
// todo: only press/release_key is implemented
use super::*;
use anyhow::Result;
use log::*;

use crate::kanata::CalculatedMouseMove;
use kanata_parser::custom_action::*;

use std::io;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::fmt;

use std::sync::Arc;
use std::sync::OnceLock;
type CbOutEvFn = dyn Fn(i64, i64, i64) -> i64 + Send + Sync + 'static; // Rust wrapper func around external callback (transmuted into this) Ahk accept only i64 arguments (vk,sc,up)
pub struct FnOutEvWrapper {
    pub cb: Arc<CbOutEvFn>,
} // wrapper struct to store our callback in a thread-shareable manner
pub static OUTEVWRAP: OnceLock<FnOutEvWrapper> = OnceLock::new(); // ensure that our wrapper struct is created once (thread-safe)

use std::sync::mpsc::{SendError, Sender as ASender};
/// Handle for writing keys to the simulated input provider.
pub struct KbdOut {
    pub tx_kout: Option<ASender<InputEvent>>,
}

use std::io::{Error as IoErr, ErrorKind::NotConnected};
impl KbdOut {
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self { tx_kout: None })
    }
    #[cfg(target_os = "linux")]
    pub fn new(
        _s: &Option<String>,
        _tp: bool,
        _name: &str,
        _bustype: evdev::BusType,
    ) -> Result<Self, io::Error> {
        Ok(Self { tx_kout: None })
    }
    #[cfg(target_os = "linux")]
    pub fn write_raw(&mut self, event: InputEvent) -> Result<(), io::Error> {
        trace!("out-raw:{event:?}");
        Ok(())
    }
    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        trace!("out:{event}");
        if let Some(tx_kout) = &self.tx_kout {
            // Send key event msg → main thread so it can be polled to try receiving it after processing external input events
            match tx_kout.send(event) {
                // send won't block for an async channel
                Ok(res) => {
                    debug!(
                        "✓ tx_kout → rx_kout@key_out(dll) ‘{event}’ from send_out_ev_msg@sim_passthru(oskbd)"
                    );
                    return Ok(res);
                }
                Err(SendError(event)) => {
                    error!(
                        "✗ tx_kout → rx_kout@key_out(dll) ‘{event}’ from send_out_ev_msg@sim_passthru(oskbd)"
                    );
                    return Err(IoErr::new(
                        NotConnected,
                        format!("Failed sending sending {event}"),
                    ));
                }
            }
        } else {
            debug!("✗ tx_kout doesn't exist");
        }
        Ok(())
    }
    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let key_ev = KeyEvent::new(key, value);
        let event = {
            #[cfg(target_os = "macos")]
            {
                key_ev.try_into().unwrap()
            }
            #[cfg(not(target_os = "macos"))]
            {
                key_ev.into()
            }
        };
        self.write(event)
    }
    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        trace!("out-code:{code};{value:?}");
        Ok(())
    }
    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }
    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        trace!("outU:{c}");
        Ok(())
    }
    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        trace!("out🖰:↓{btn:?}");
        Ok(())
    }
    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        trace!("out🖰:↑{btn:?}");
        Ok(())
    }
    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        trace!("scroll:{direction:?},{distance:?}");
        Ok(())
    }
    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        let (direction, distance) = (mv.direction, mv.distance);
        trace!("out🖰:move {direction:?},{distance:?}");
        Ok(())
    }
    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        for mv in moves {
            let (direction, distance) = (&mv.direction, &mv.distance);
            trace!("out🖰:move {direction:?},{distance:?}");
        }
        Ok(())
    }
    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        log::info!("out🖰:@{x},{y}");
        Ok(())
    }
    pub fn tick(&mut self) {}
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub code: u32,

    /// Key was released
    pub up: bool,
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use kanata_keyberon::key_code::KeyCode;
        let direction = if self.up { "↑" } else { "↓" };
        let key_name = KeyCode::from(OsCode::from(self.code));
        write!(f, "{}{:?}", direction, key_name)
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl InputEvent {
    pub fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        Self {
            code: code.into(),
            up: val.into(),
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();
    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        Ok(Self {
            code: OsCode::from_u16(item.code as u16).ok_or(())?,
            value: match item.up {
                true => KeyValue::Release,
                false => KeyValue::Press,
            },
        })
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        Self {
            code: item.code.into(),
            up: item.value.into(),
        }
    }
}
