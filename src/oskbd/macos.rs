//! Contains the input/output code for keyboards on Macos.

use super::*;
use crate::kanata::CalculatedMouseMove;
use crate::oskbd::KeyEvent;
use anyhow::Error;
use driverkit::KeyEvent as dKeyEvent;
use driverkit::{send_key, wait_key, grab_kb};
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;
use rustc_hash::FxHashMap as HashMap;
use std::convert::TryFrom;
use std::io;

//  see the whole discove devices thing, might be needed here for macos

pub const HI_RES_SCROLL_UNITS_IN_LO_RES: u16 = 120;

#[derive(Debug, Clone, Copy)]
pub struct InputEvent { value: u64, page: u32, code: u32, }

impl InputEvent {
    pub fn new(event: dKeyEvent) -> Self {
        InputEvent { 
            value: event.value, 
            page: event.page, 
            code: event.code, }
    }
    pub fn to_driverkit_event(self) -> dKeyEvent {
        dKeyEvent { 
            value: self.value, 
            page:  self.page, 
            code:  self.code, }
    }
}

pub struct KbdIn {}

impl KbdIn {
    pub fn new() -> Result<Self, io::Error> {
        let grab_status = grab_kb("Karabiner DriverKit VirtualHIDKeyboard 1.7.0");
        if grab_status == 0 { 
            Ok(Self {}) 
        } else { 
            Err(io::Error::new(io::ErrorKind::NotConnected, "Couldn't grab keyboard" )) 
        }
    }

    pub fn read(&mut self) -> Result<InputEvent, io::Error> {
        // looks like read event
        let mut event = dKeyEvent { value: 0, page: 0, code: 0, };
        wait_key(&mut event);
        Ok( InputEvent {
            value: event.value,
            page: event.page,
            code: event.code
        }
        )
    }

}

impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();

    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        if let Some(oscode) = OsCode::from_u16( (item.page << 8 | item.code) as u16 ) {
            Ok(KeyEvent {
                code: oscode,
                value: if item.value == 1 {
                    KeyValue::Press
                } else {
                    KeyValue::Release
                    // nano: error on other than 1 or 0
                },
            })
        } else {
            Err(())
        }
    }
}

impl From<KeyEvent> for InputEvent {

    fn from(item: KeyEvent) -> Self {
        let val = match item.value {
            KeyValue::Press => 1,
            _ => 0,
            // nano what about tap and others?
        };
        InputEvent {
            value: val,
            page: ( (item.code as u16 & 0xFF00u16) >> 2) as u32,
            code: ( item.code as u16 & 0x00FFu16 ) as u32,
        }
    }

}

pub struct KbdOut {
    // check windows, i dont think anything is needed here
}

impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        // i dont think there's much to do!
        Ok(KbdOut {})
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {

        let mut devent = event.to_driverkit_event();
        let sent = send_key(&mut devent);

        if sent != 0 { 
            Ok(()) 
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "ay haga " ))  // nano
        }
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        self.write( InputEvent::from(KeyEvent{value, code: key}) )
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        self.write( InputEvent::from(KeyEvent{value, code: OsCode::from_u16(code as u16).unwrap()}) )
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
        // used, check windows and linux
        todo!();
    }

    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        Ok(())
    }

    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        Ok(())
    }
}
