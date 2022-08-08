//! Contains the input/output code for keyboards on Linux.

use evdev::{uinput, Device, EventType, InputEvent, RelativeAxisType};
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};

use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::thread;

use crate::custom_action::*;
use crate::keys::KeyEvent;
use crate::keys::*;

pub struct KbdIn {
    devices: HashMap<Token, Device>,
    poll: Poll,
    events: Events,
}

impl KbdIn {
    pub fn new(dev_paths: &str) -> Result<Self, std::io::Error> {
        match KbdIn::new_linux(dev_paths) {
            Ok(s) => Ok(s),
            Err(e) => {
                log::error!("Failed to open the input keyboard device. Make sure you've added kanata to the `input` group. E: {}", e);
                Err(e)
            }
        }
    }

    fn new_linux(dev_paths: &str) -> Result<Self, std::io::Error> {
        let mut devices = HashMap::new();
        let poll = Poll::new()?;
        for (i, dev_path) in dev_paths.split(':').enumerate() {
            let mut kbd_in_dev = Device::open(dev_path)?;

            // NOTE: This grab-ungrab-grab sequence magically
            // fix an issue with a Lenovo Yoga trackpad not working.
            // No idea why this works.
            kbd_in_dev.grab()?;
            kbd_in_dev.ungrab()?;
            kbd_in_dev.grab()?;

            let tok = Token(i);
            let fd = kbd_in_dev.as_raw_fd();
            poll.registry()
                .register(&mut SourceFd(&fd), tok, Interest::READABLE)?;
            devices.insert(tok, kbd_in_dev);
        }
        if devices.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No valid keyboard devices provided",
            ));
        }

        let events = Events::with_capacity(32);
        Ok(KbdIn {
            devices,
            poll,
            events,
        })
    }

    pub fn read(&mut self) -> Result<Vec<InputEvent>, std::io::Error> {
        let mut input_events = vec![];
        loop {
            log::trace!("polling");
            if let Err(e) = self.poll.poll(&mut self.events, None) {
                log::error!("failed poll: {:?}", e);
                return Ok(vec![]);
            }
            for event in &self.events {
                if let Some(device) = self.devices.get_mut(&event.token()) {
                    device
                        .fetch_events()
                        .map_err(|e| {
                            log::error!("failed fetch events");
                            e
                        })?
                        .into_iter()
                        .for_each(|ev| input_events.push(ev));
                } else {
                    panic!("encountered unexpected epoll event {event:?}");
                }
            }
            if !input_events.is_empty() {
                return Ok(input_events);
            }
        }
    }
}

pub struct KbdOut {
    device: uinput::VirtualDevice,
    accumulated_scroll: u16,
    accumulated_hscroll: u16,
    #[allow(dead_code)] // stored here for persistence+cleanup on exit
    symlink: Option<Symlink>,
}

pub const HI_RES_SCROLL_UNITS_IN_LO_RES: u16 = 120;

impl KbdOut {
    pub fn new(symlink_path: &Option<String>) -> Result<Self, io::Error> {
        let mut keys = evdev::AttributeSet::new();
        for k in 0..300u16 {
            keys.insert(evdev::Key(k));
        }
        let mut axes = evdev::AttributeSet::new();
        axes.insert(RelativeAxisType::REL_WHEEL);
        axes.insert(RelativeAxisType::REL_HWHEEL);

        let mut device = uinput::VirtualDeviceBuilder::new()?
            .name("kanata")
            .input_id(evdev::InputId::new(evdev::BusType::BUS_USB, 1, 1, 1))
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()?;
        let devnode = device
            .enumerate_dev_nodes_blocking()?
            .next() // Expect only one. Using fold or calling next again blocks indefinitely
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "devnode is not found"))??;
        log::info!("Created device {:#?}", devnode);
        let symlink = if let Some(symlink_path) = symlink_path {
            let dest = PathBuf::from(symlink_path);
            let symlink = Symlink::new(devnode, dest)?;
            Symlink::clean_when_killed(symlink.clone());
            Some(symlink)
        } else {
            None
        };

        Ok(KbdOut {
            device,
            accumulated_scroll: 0,
            accumulated_hscroll: 0,
            symlink,
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

    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        log::debug!("scroll: {direction:?} {distance:?}");
        match direction {
            MWheelDirection::Up | MWheelDirection::Down => {
                let lo_res_distance = distance / HI_RES_SCROLL_UNITS_IN_LO_RES;
                if lo_res_distance > 0 {
                    self.do_scroll(direction, lo_res_distance)?;
                }
                let leftover_scroll = distance % HI_RES_SCROLL_UNITS_IN_LO_RES;
                if leftover_scroll > 0 {
                    self.accumulated_scroll += leftover_scroll;
                    if self.accumulated_scroll >= HI_RES_SCROLL_UNITS_IN_LO_RES {
                        self.accumulated_scroll -= HI_RES_SCROLL_UNITS_IN_LO_RES;
                        self.do_scroll(direction, 1)?;
                    }
                }
            }
            MWheelDirection::Left | MWheelDirection::Right => {
                let lo_res_distance = distance / HI_RES_SCROLL_UNITS_IN_LO_RES;
                if lo_res_distance > 0 {
                    self.do_hscroll(direction, lo_res_distance)?;
                }
                let leftover_scroll = distance % HI_RES_SCROLL_UNITS_IN_LO_RES;
                if leftover_scroll > 0 {
                    self.accumulated_hscroll += leftover_scroll;
                    if self.accumulated_hscroll >= HI_RES_SCROLL_UNITS_IN_LO_RES {
                        self.accumulated_hscroll -= HI_RES_SCROLL_UNITS_IN_LO_RES;
                        self.do_hscroll(direction, 1)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn do_scroll(
        &mut self,
        direction: MWheelDirection,
        lo_res_distance: u16,
    ) -> Result<(), io::Error> {
        let ev = InputEvent::new(
            EventType::RELATIVE,
            RelativeAxisType::REL_WHEEL.0,
            match direction {
                MWheelDirection::Up => i32::from(lo_res_distance),
                MWheelDirection::Down => -i32::from(lo_res_distance),
                _ => unreachable!(), // unreachable based on pub fn scroll
            },
        );
        self.write(ev)
    }

    fn do_hscroll(
        &mut self,
        direction: MWheelDirection,
        lo_res_distance: u16,
    ) -> Result<(), io::Error> {
        let ev = InputEvent::new(
            EventType::RELATIVE,
            RelativeAxisType::REL_HWHEEL.0,
            match direction {
                MWheelDirection::Right => i32::from(lo_res_distance),
                MWheelDirection::Left => -i32::from(lo_res_distance),
                _ => unreachable!(), // unreachable based on pub fn scroll
            },
        );
        self.write(ev)
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
