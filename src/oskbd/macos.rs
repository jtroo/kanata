//! Contains the input/output code for keyboards on Macos.
use super::*;
use crate::kanata::CalculatedMouseMove;
use crate::oskbd::KeyEvent;
use anyhow::anyhow;
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;
use karabiner_driverkit::*;
use std::convert::TryFrom;
use std::io;


pub const HI_RES_SCROLL_UNITS_IN_LO_RES: u16 = 120;

#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    value: u64,
    page: u32,
    code: u32,
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

impl KbdIn {
    pub fn new(devices: &[String]) -> Result<Self, anyhow::Error> {
        if !driver_activated() {
            return Err(anyhow!(
                "Karabiner-VirtualHIDDevice driver is not activated."
            ));
        }

        let registered_devices = if !devices.is_empty() {
            validate_and_register_devices(devices)
        } else {
            vec![]
        };

        if !registered_devices.is_empty() || register_device("") {
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

fn validate_and_register_devices(devices: &[String]) -> Vec<String> {
    devices
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

pub struct KbdOut {}

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

    /// Send using C-S-u + <unicode hex number> + spc
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        log::debug!("sending unicode {c}");
        todo!();
    }

    pub fn scroll(&mut self, _direction: MWheelDirection, _distance: u16) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn click_btn(&mut self, _btn: Btn) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn release_btn(&mut self, _btn: Btn) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn move_mouse(&mut self, _mv: CalculatedMouseMove) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn move_mouse_many(&mut self, _moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn set_mouse(&mut self, _x: u16, _y: u16) -> Result<(), io::Error> {
        Ok(())
    }
}
