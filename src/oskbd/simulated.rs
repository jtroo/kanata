//! Output that just prints text to stdout instead of actually doing anything OS-related.

use super::*;

use crate::kanata::CalculatedMouseMove;
use kanata_parser::custom_action::*;

#[cfg(target_os = "linux")]
use evdev::InputEvent;

use std::io;

/// Handle for writing keys to the OS.
pub struct KbdOut {}

impl KbdOut {
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {})
    }

    #[cfg(target_os = "linux")]
    pub fn new(_: &Option<String>) -> Result<Self, io::Error> {
        Ok(Self {})
    }

    #[cfg(target_os = "linux")]
    pub fn write_raw(&mut self, event: InputEvent) -> Result<(), io::Error> {
        println!("rawevent:{event:?}");
        Ok(())
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        println!("out:{event:?}");
        Ok(())
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let key_ev = KeyEvent::new(key, value);
        let event = key_ev.into();
        self.write(event)
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        println!("out-code:{code};{value:?}");
        Ok(())
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        println!("unicode:{c}");
        Ok(())
    }

    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        println!("mouse-press:{btn:?}");
        Ok(())
    }

    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        println!("mouse-release:{btn:?}");
        Ok(())
    }

    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        println!("scroll:{direction:?},{distance:?}");
        Ok(())
    }

    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        let (direction, distance) = (mv.direction, mv.distance);
        println!("mouse-move:{direction:?},{distance:?}");
        Ok(())
    }

    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        for mv in moves {
            let (direction, distance) = (&mv.direction, &mv.distance);
            println!("mouse-move:{direction:?},{distance:?}");
        }
        Ok(())
    }

    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        log::info!("set-mouse:{x},{y}");
        Ok(())
    }
}
