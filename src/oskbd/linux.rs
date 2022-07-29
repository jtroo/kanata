//! Contains the input/output code for keyboards on Linux.

use evdev_rs::enums;
use evdev_rs::enums::BusType;
use evdev_rs::enums::EventCode;
use evdev_rs::enums::EventType;
use evdev_rs::enums::EV_SYN;
use evdev_rs::Device;
use evdev_rs::DeviceWrapper;
use evdev_rs::GrabMode;
use evdev_rs::InputEvent;
use evdev_rs::ReadFlag;
use evdev_rs::ReadStatus;
use evdev_rs::TimeVal;
use evdev_rs::UInputDevice;
use evdev_rs::UninitDevice;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;

use crate::custom_action::*;
use crate::keys::*;

use std::fs;
use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::thread;

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
    device: UInputDevice,
    _symlink: Option<Symlink>,
}

impl KbdOut {
    pub fn new(symlink_path: &Option<String>) -> Result<Self, io::Error> {
        let device = UninitDevice::new()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "UninitDevice::new() failed"))?;

        device.set_name("kanata");
        device.set_bustype(BusType::BUS_USB as u16);
        device.set_vendor_id(0x1);
        device.set_product_id(0x1);
        device.set_version(1);

        device.enable(EventType::EV_SYN)?;
        device.enable(EventType::EV_KEY)?;
        for key in (0..300).into_iter().filter_map(enums::int_to_ev_key) {
            device.enable(EventCode::EV_KEY(key))?;
        }

        let device = UInputDevice::create_from_device(&device)?;

        let devnode = device
            .devnode()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "devnode is not found"))?;
        log::info!("Created device {:#?}", devnode);
        let symlink = if let Some(symlink_path) = symlink_path {
            let dest = PathBuf::from(symlink_path);
            let symlink = Symlink::new(PathBuf::from(devnode), dest)?;
            Symlink::clean_when_killed(symlink.clone());
            Some(symlink)
        } else {
            None
        };

        Ok(KbdOut {
            device,
            _symlink: symlink,
        })
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        self.device.write_event(&event)?;
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

#[derive(Clone)]
struct Symlink {
    dest: PathBuf,
}

impl Symlink {
    fn new(source: PathBuf, dest: PathBuf) -> Result<Self, io::Error> {
        if let Ok(metadata) = fs::symlink_metadata(&dest) {
            if metadata.file_type().is_symlink() {
                fs::remove_file(&dest)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Cannot create a symlink at \"{}\": path already exists.",
                        dest.to_string_lossy()
                    ),
                ));
            }
        }
        std::os::unix::fs::symlink(&source, &dest)?;
        log::info!("Created symlink {:#?} -> {:#?}", dest, source);
        Ok(Self { dest })
    }

    fn clean_when_killed(symlink: Self) {
        thread::spawn(|| {
            let mut signals = Signals::new(&[SIGINT, SIGTERM]).unwrap();
            for signal in &mut signals {
                match signal {
                    SIGINT | SIGTERM => {
                        drop(symlink);
                        signal_hook::low_level::emulate_default_handler(signal).unwrap();
                        unreachable!();
                    }
                    _ => unreachable!(),
                }
            }
        });
    }
}

impl Drop for Symlink {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.dest);
        log::info!("Deleted symlink {:#?}", self.dest);
    }
}
