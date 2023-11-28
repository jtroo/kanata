//! Contains the input/output code for keyboards on Macos.

use rustc_hash::FxHashMap as HashMap;
use crate::kanata::CalculatedMouseMove;
use std::convert::TryFrom;
use std::io;
use super::*;
use crate::oskbd::KeyEvent;
use kanata_parser::keys::*;
use kanata_parser::custom_action::*;


//  see the whole discove devices thing, might be needed here for macos

pub const HI_RES_SCROLL_UNITS_IN_LO_RES: u16 = 120;
struct InputEvent {}


pub struct KbdIn {

    // devices: HashMap<Token, (Device, String)>,
    // /// Some(_) if devices are explicitly listed, otherwise None.
    // missing_device_paths: Option<Vec<String>>,
    // poll: Poll,
    // events: Events,
    // token_counter: usize,
    // /// stored to prevent dropping
    // _inotify: Inotify,
    // include_names: Option<Vec<String>>,
    // exclude_names: Option<Vec<String>>,

}

impl KbdIn {
    pub fn new(

        // dev_paths: &[String],
        // continue_if_no_devices: bool,
        // include_names: Option<Vec<String>>,
        // exclude_names: Option<Vec<String>>,

    ) -> Result<Self, io::Error> {
        // my understanding is this is our grab
        // see windows' kbdin::new()
        todo!();
    }

    pub fn read(&mut self) -> Result<Vec<InputEvent>, io::Error> {
        // looks like read event
        todo!();
    }
}



impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();
    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        // i think here we convert from dext keyevent to kanata's
        todo!();
    }
}

impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        // here from kanata's keyevent to dext's
        todo!();
    }
}

pub struct KbdOut {


    // check windows, i dont think anything is needed here

    // device: uinput::VirtualDevice,
    // accumulated_scroll: u16,
    // accumulated_hscroll: u16,
    // #[allow(dead_code)] // stored here for persistence+cleanup on exit
    // symlink: Option<Symlink>,
    // raw_buf: Vec<InputEvent>,
    // pub unicode_termination: Cell<UnicodeTermination>,
    // pub unicode_u_code: Cell<OsCode>,

}


impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {

        // i dont think there's much to do!
        todo!();

        Ok(KbdOut {
            // device,
            // accumulated_scroll: 0,
            // accumulated_hscroll: 0,
            // symlink,
            // raw_buf: vec![],
            //
            // // historically was the only option, so make Enter the default
            // unicode_termination: Cell::new(UnicodeTermination::Enter),
            //
            // // historically was the only option, so make KEY_U the default
            // unicode_u_code: Cell::new(OsCode::KEY_U),
        })

    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        // don't know what is this tbh, maybe reflect input event as is? 
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
