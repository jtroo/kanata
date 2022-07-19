// This file contains the original ktrl project's `kbd_in.rs` and `kbd_out.rs` files.

use evdev_rs::enums::EventCode;
use evdev_rs::enums::EV_SYN;
use evdev_rs::Device;
use evdev_rs::GrabMode;
use evdev_rs::InputEvent;
use evdev_rs::ReadFlag;
use evdev_rs::ReadStatus;
use evdev_rs::TimeVal;

use uinput_sys::uinput_user_dev;

use crate::custom_action::*;
use crate::keys::*;
use libc::c_char;
use libc::input_event as raw_event;

// file i/o
use io::Write;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::Path;

// unsafe
use std::mem;
use std::slice;

// kanata
use crate::keys::KeyEvent;

pub struct KbdIn {
    device: Device,
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
        let kbd_in_file = File::open(dev_path)?;
        let mut kbd_in_dev = Device::new_from_file(kbd_in_file)?;

        // NOTE: This grab-ungrab-grab sequence magically
        // fix an issue I had with my Lenovo Yoga trackpad not working.
        // I honestly have no idea why this works haha.
        kbd_in_dev.grab(GrabMode::Grab)?;
        kbd_in_dev.grab(GrabMode::Ungrab)?;
        kbd_in_dev.grab(GrabMode::Grab)?;

        Ok(KbdIn { device: kbd_in_dev })
    }

    pub fn read(&self) -> Result<InputEvent, std::io::Error> {
        let (status, event) = self
            .device
            .next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING)?;
        std::assert!(status == ReadStatus::Success);
        Ok(event)
    }
}

pub struct KbdOut {
    device: File,
}

impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        let mut uinput_out_file = OpenOptions::new().write(true).open("/dev/uinput")?;

        unsafe {
            let rc = uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_SYN);
            if rc != 0 {
                log::error!("ui_set_evbit for EV_SYN returned {}", rc);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "ui_set_evbit failed for EV_SYN",
                ));
            }
            let rc = uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_KEY);
            if rc != 0 {
                log::error!("ui_set_evbit for EV_KEY returned {}", rc);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "ui_set_evbit failed for EV_KEY",
                ));
            }

            for key in 0..300 {
                let rc = uinput_sys::ui_set_keybit(uinput_out_file.as_raw_fd(), key);
                if rc != 0 {
                    log::error!("ui_set_keybit for {} returned {}", key, rc);
                    return Err(io::Error::new(io::ErrorKind::Other, "ui_set_keybit failed"));
                }
            }

            let mut uidev: uinput_user_dev = mem::zeroed();

            const PROG_NAME: &[u8] = "kanata".as_bytes();
            let copy_len = std::cmp::min(PROG_NAME.len(), uidev.name.len());
            assert!(copy_len <= uidev.name.len());
            for (i, c) in PROG_NAME.iter().copied().enumerate().take(copy_len) {
                uidev.name[i] = c as c_char;
            }

            uidev.id.bustype = 0x3; // BUS_USB
            uidev.id.vendor = 0x1;
            uidev.id.product = 0x1;
            uidev.id.version = 1;

            let uidev_bytes =
                slice::from_raw_parts(mem::transmute(&uidev), mem::size_of::<uinput_user_dev>());
            uinput_out_file.write_all(uidev_bytes)?;
            let rc = uinput_sys::ui_dev_create(uinput_out_file.as_raw_fd());
            if rc != 0 {
                log::error!("ui_dev_create for returned {}", rc);
                return Err(io::Error::new(io::ErrorKind::Other, "ui_dev_create failed"));
            }
        }

        Ok(KbdOut {
            device: uinput_out_file,
        })
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        let ev = event.as_raw();

        unsafe {
            let ev_bytes = slice::from_raw_parts(
                mem::transmute(&ev as *const raw_event),
                mem::size_of::<raw_event>(),
            );
            self.device.write_all(ev_bytes)?;
        };

        Ok(())
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let key_ev = KeyEvent::new(key, value);
        let input_ev = key_ev.into();
        log::debug!("input ev: {:?}", input_ev);
        self.write(input_ev)?;

        let sync = InputEvent::new(
            &TimeVal {
                tv_sec: 0,
                tv_usec: 0,
            },
            &EventCode::EV_SYN(EV_SYN::SYN_REPORT),
            0,
        );
        self.write(sync)?;

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
