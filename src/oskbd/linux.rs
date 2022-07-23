// This file contains the original ktrl project's `kbd_in.rs` and `kbd_out.rs` files.

use evdev::uinput;
use evdev::Device;
use evdev::InputEvent;

use crate::custom_action::*;
use crate::keys::*;

use std::io;
use std::path::Path;

// kanata
use crate::keys::KeyEvent;

pub struct KbdIn {
    device: Device,
    events: std::collections::VecDeque<InputEvent>,
}

impl KbdIn {
    pub fn new(dev_path: &Path) -> Result<Self, std::io::Error> {
        match KbdIn::new_linux(dev_path) {
            Ok(s) => Ok(s),
            Err(e) => {
                log::error!("Failed to open the input keyboard device. Make sure you've added kanata to the `input` group. E: {}", e);
                Err(e)
            }
        }
    }

    fn new_linux(dev_path: &Path) -> Result<Self, std::io::Error> {
        let mut kbd_in_dev = Device::open(dev_path)?;

        // NOTE: This grab-ungrab-grab sequence magically
        // fix an issue with a Lenovo Yoga trackpad not working.
        // No idea why this works.
        kbd_in_dev.grab()?;
        kbd_in_dev.ungrab()?;
        kbd_in_dev.grab()?;

        Ok(KbdIn {
            device: kbd_in_dev,
            events: Default::default(),
        })
    }

    pub fn read(&mut self) -> Result<InputEvent, std::io::Error> {
        while self.events.is_empty() {
            self.device
                .fetch_events()?
                .into_iter()
                .for_each(|ev| self.events.push_back(ev));
        }
        Ok(self.events.pop_front().expect("empty events"))
    }
}

pub struct KbdOut {
    device: uinput::VirtualDevice,
}

impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        let mut keys = evdev::AttributeSet::new();
        for k in 0..300u16 {
            keys.insert(evdev::Key(k));
        }
        Ok(KbdOut {
            device: uinput::VirtualDeviceBuilder::new()?
                .name("kanata")
                .input_id(evdev::InputId::new(evdev::BusType::BUS_USB, 1, 1, 1))
                .with_keys(&keys)?
                .build()?,
        })
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        self.device.emit(&[event])?;
        Ok(())
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let key_ev = KeyEvent::new(key, value);
        let input_ev = key_ev.into();
        log::debug!("input ev: {:?}", input_ev);
        self.device.emit(&[input_ev])?;
        Ok(())
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    /// Send using C-S-u + <unicode hex number> + ret
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        let hex = format!("{:x}", c as u32);
        self.press_key(OsCode::KEY_LEFTCTRL)?;
        self.press_key(OsCode::KEY_LEFTSHIFT)?;
        self.press_key(OsCode::KEY_U)?;
        self.release_key(OsCode::KEY_U)?;
        self.release_key(OsCode::KEY_LEFTSHIFT)?;
        self.release_key(OsCode::KEY_LEFTCTRL)?;
        let mut s = String::new();
        for c in hex.chars() {
            s.push(c);
            let osc = str_to_oscode(&s).expect("invalid char in unicode output");
            s.clear();
            self.press_key(osc)?;
            self.release_key(osc)?;
        }
        self.press_key(OsCode::KEY_ENTER)?;
        self.release_key(OsCode::KEY_ENTER)?;
        Ok(())
    }

    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        self.press_key(btn.into())
    }

    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        self.release_key(btn.into())
    }
}

impl From<Btn> for OsCode {
    fn from(btn: Btn) -> Self {
        match btn {
            Btn::Left => OsCode::BTN_LEFT,
            Btn::Right => OsCode::BTN_RIGHT,
            Btn::Mid => OsCode::BTN_MIDDLE,
        }
    }
}
