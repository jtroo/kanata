use evdev_rs::Device;
use evdev_rs::GrabMode;
use evdev_rs::InputEvent;
use evdev_rs::ReadFlag;
use evdev_rs::ReadStatus;

use std::fs::File;
use std::path::Path;

pub struct KbdIn {
    device: Device,
}

impl KbdIn {
    pub fn new(dev_path: &Path) -> Result<Self, std::io::Error> {
        match KbdIn::new_linux(dev_path) {
            Ok(s) => Ok(s),
            Err(e) => {
                log::error!("Failed to open the input keyboard device. Make sure you've added ktrl to the `input` group. E: {}", e);
                Err(e)
            }
        }
    }

    fn new_linux(dev_path: &Path) -> Result<Self, std::io::Error> {
        let kbd_in_file = File::open(dev_path)?;
        let mut kbd_in_dev = Device::new_from_fd(kbd_in_file)?;

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

use uinput_sys::uinput_user_dev;

use crate::keys::KeyValue;
use crate::keys::OsCode;
use evdev_rs::enums::EventCode;
use evdev_rs::enums::EV_SYN;
use evdev_rs::TimeVal;
use libc::input_event as raw_event;

// file i/o
use io::Write;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;

// unsafe
use std::mem;
use std::slice;

// ktrl
use crate::keys::KeyEvent;

pub struct KbdOut {
    device: File,
}

impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        let mut uinput_out_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")?;

        unsafe {
            uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_SYN);
            uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_KEY);

            for key in 0..uinput_sys::KEY_MAX {
                uinput_sys::ui_set_keybit(uinput_out_file.as_raw_fd(), key);
            }

            let mut uidev: uinput_user_dev = mem::zeroed();
            uidev.name[0] = 'k' as i8;
            uidev.name[1] = 't' as i8;
            uidev.name[2] = 'r' as i8;
            uidev.name[3] = 'l' as i8;
            uidev.id.bustype = 0x3; // BUS_USB
            uidev.id.vendor = 0x1;
            uidev.id.product = 0x1;
            uidev.id.version = 1;

            let uidev_bytes =
                slice::from_raw_parts(mem::transmute(&uidev), mem::size_of::<uinput_user_dev>());
            uinput_out_file.write_all(uidev_bytes)?;
            uinput_sys::ui_dev_create(uinput_out_file.as_raw_fd());
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
        self.write(key_ev.into())?;

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
}
