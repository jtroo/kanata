//! Contains the input/output code for keyboards on Macos.

use super::*;
use crate::kanata::CalculatedMouseMove;
use crate::oskbd::KeyEvent;
use anyhow::Error;
use driverkit::KeyEvent as dKeyEvent;
use driverkit::{send_key, wait_key};
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;
use rustc_hash::FxHashMap as HashMap;
use std::convert::TryFrom;
use std::io;

//  see the whole discove devices thing, might be needed here for macos

pub const HI_RES_SCROLL_UNITS_IN_LO_RES: u16 = 120;

struct InputEvent {
    value: u64,
    page: u32,
    code: u32,
}

pub struct KbdIn {}

impl KbdIn {
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {})
    }

    // pub fn read(&mut self) -> Result<Vec<InputEvent>, io::Error> {
    //     // looks like read event
    //     let mut event = dKeyEvent { value: 0, page: 0, code: 0, };
    //     let _key = wait_key(&mut event);
    //     Ok(InputEvent {
    //         value: event.value,
    //         code: (event.page << 8 | event.code) as u16
    //     })
    // }
}

impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();

    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        if let Some(oscode) = OsCode::from_u16((item.page < 8 | item.code) as u16) {
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

impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        let val = match item.value {
            KeyValue::Press => 1,
            _ => 0,
        };
        InputEvent {
            value: val,
            code: item.code.as_u16(),
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
        todo!();
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        // think this is write_event right here
        todo!();
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        // think this is write_event right here
        todo!();
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    /// Send using C-S-u + <unicode hex number> + spc
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
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
