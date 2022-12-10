//! Contains the input/output code for keyboards on Linux.

use evdev::{uinput, Device, EventType, InputEvent, RelativeAxisType};
use inotify::{Inotify, WatchMask};
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use nix::ioctl_read_buf;
use rustc_hash::FxHashMap as HashMap;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};

use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::thread;

use crate::custom_action::*;
use crate::keys::KeyEvent;
use crate::keys::*;

pub struct KbdIn {
    devices: HashMap<Token, (Device, String)>,
    /// Some(_) if devices are explicitly listed, otherwise None.
    missing_device_paths: Option<Vec<String>>,
    poll: Poll,
    events: Events,
    token_counter: usize,
    /// stored to prevent dropping
    _inotify: Inotify,
}

const INOTIFY_TOKEN_VALUE: usize = 0;
const INOTIFY_TOKEN: Token = Token(INOTIFY_TOKEN_VALUE);

impl KbdIn {
    pub fn new(dev_paths: &[String], continue_if_no_devices: bool) -> Result<Self, io::Error> {
        let poll = Poll::new()?;

        let mut missing_device_paths = None;
        let devices = if !dev_paths.is_empty() {
            missing_device_paths = Some(vec![]);
            devices_from_input_paths(dev_paths, missing_device_paths.as_mut().unwrap())
        } else {
            discover_devices()?
        };
        if devices.is_empty() {
            if continue_if_no_devices {
                log::warn!("no keyboard devices found; kanata is waiting");
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "No keyboard devices were found",
                ));
            }
        }
        let _inotify = watch_devinput().map_err(|e| {
            log::error!("failed to watch files: {e:?}");
            e
        })?;
        poll.registry().register(
            &mut SourceFd(&_inotify.as_raw_fd()),
            INOTIFY_TOKEN,
            Interest::READABLE,
        )?;

        let mut kbdin = Self {
            poll,
            missing_device_paths,
            _inotify,
            events: Events::with_capacity(32),
            devices: HashMap::default(),
            token_counter: INOTIFY_TOKEN_VALUE + 1,
        };

        for (device, dev_path) in devices.into_iter() {
            if let Err(e) = kbdin.register_device(device, dev_path.clone()) {
                log::warn!("found device {dev_path} but could not register it {e:?}");
                if let Some(ref mut missing) = kbdin.missing_device_paths {
                    missing.push(dev_path);
                }
            }
        }

        Ok(kbdin)
    }

    fn register_device(&mut self, mut dev: Device, path: String) -> Result<(), io::Error> {
        log::info!("registering {path}");
        wait_for_all_keys_unpressed(&dev)?;
        // NOTE: This grab-ungrab-grab sequence magically fixes an issue with a Lenovo Yoga
        // trackpad not working. No idea why this works.
        dev.grab()?;
        dev.ungrab()?;
        dev.grab()?;

        let tok = Token(self.token_counter);
        self.token_counter += 1;
        let fd = dev.as_raw_fd();
        self.poll
            .registry()
            .register(&mut SourceFd(&fd), tok, Interest::READABLE)?;
        self.devices.insert(tok, (dev, path));
        Ok(())
    }

    pub fn read(&mut self) -> Result<Vec<InputEvent>, io::Error> {
        let mut input_events = vec![];
        loop {
            log::trace!("polling");

            if let Err(e) = self.poll.poll(&mut self.events, None) {
                log::error!("failed poll: {:?}", e);
                return Ok(vec![]);
            }

            let mut do_rediscover = false;
            for event in &self.events {
                if let Some((device, _)) = self.devices.get_mut(&event.token()) {
                    if let Err(e) = device
                        .fetch_events()
                        .map(|evs| evs.into_iter().for_each(|ev| input_events.push(ev)))
                    {
                        // Currently the kind() is uncategorized... not helpful, need to match
                        // on os error (19)
                        match e.raw_os_error() {
                            Some(19) => {
                                self.poll
                                    .registry()
                                    .deregister(&mut SourceFd(&device.as_raw_fd()))?;
                                if let Some((_, path)) = self.devices.remove(&event.token()) {
                                    log::warn!("removing kbd device: {path}");
                                    if let Some(ref mut missing) = self.missing_device_paths {
                                        missing.push(path);
                                    }
                                }
                            }
                            _ => {
                                log::error!("failed fetch events due to {e}, kind: {}", e.kind());
                                return Err(e);
                            }
                        };
                    }
                } else if event.token() == INOTIFY_TOKEN {
                    do_rediscover = true;
                } else {
                    panic!("encountered unexpected epoll event {event:?}");
                }
            }
            if do_rediscover {
                log::info!("watch found file changes, looking for new devices");
                self.rediscover_devices()?;
            }
            if !input_events.is_empty() {
                return Ok(input_events);
            }
        }
    }

    fn rediscover_devices(&mut self) -> Result<(), io::Error> {
        // This function is kinda ugly but the borrow checker doesn't like all this mutation.
        let mut paths_registered = vec![];
        if let Some(ref mut missing) = self.missing_device_paths {
            if missing.is_empty() {
                log::info!("no devices are missing, doing nothing");
                return Ok(());
            }
            log::info!("checking for {missing:?}");
            let discovered_devices = missing
                .iter()
                .filter_map(|dev_path| {
                    for _ in 0..10 {
                        // try a few times with waits in between; device might not be ready
                        if let Ok(device) = Device::open(dev_path) {
                            return Some((device, dev_path.clone()));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    None
                })
                .collect::<Vec<(_, _)>>();
            for (device, dev_path) in discovered_devices {
                if let Err(e) = self.register_device(device, dev_path.clone()) {
                    log::warn!("found device {dev_path} but could not register it {e:?}");
                } else {
                    paths_registered.push(dev_path);
                }
            }
        }
        if let Some(ref mut missing) = self.missing_device_paths {
            missing.retain(|path| !paths_registered.contains(path));
        } else {
            discover_devices()?
                .into_iter()
                .try_for_each(|(dev, path)| {
                    if !self
                        .devices
                        .values()
                        .any(|(_, registered_path)| &path == registered_path)
                    {
                        self.register_device(dev, path)
                    } else {
                        Ok(())
                    }
                })?;
        }
        Ok(())
    }
}

pub fn is_input_device(device: &Device) -> bool {
    use evdev::Key;
    let is_keyboard = device
        .supported_keys()
        .map_or(false, |keys| keys.contains(Key::KEY_ENTER));
    let is_mouse = device
        .supported_relative_axes()
        .map_or(false, |axes| axes.contains(RelativeAxisType::REL_X));
    if is_keyboard || is_mouse {
        if device.name() == Some("kanata") {
            return false;
        }
        log::debug!(
            "Detected {}: name={} physical_path={:?}",
            if is_keyboard && is_mouse {
                "Keyboard/Mouse"
            } else if is_keyboard {
                "Keyboard"
            } else {
                "Mouse"
            },
            device.name().unwrap(),
            device.physical_path()
        );
        true
    } else {
        log::trace!("Detected other device: {}", device.name().unwrap());
        false
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
        // Support pretty much every feature of a Keyboard or a Mouse in a VirtualDevice so that no event from the original input devices gets lost
        // TODO investigate the rare possibility that a device is e.g. a Joystick and a Keyboard or a Mouse at the same time, which could lead to lost events

        // For some reason 0..0x300 (max value for a key) doesn't work, the closest that I've got to work is 560
        let keys = evdev::AttributeSet::from_iter((0..560).map(evdev::Key));
        let relative_axes = evdev::AttributeSet::from_iter([
            RelativeAxisType::REL_WHEEL,
            RelativeAxisType::REL_HWHEEL,
            RelativeAxisType::REL_X,
            RelativeAxisType::REL_Y,
            RelativeAxisType::REL_Z,
            RelativeAxisType::REL_RX,
            RelativeAxisType::REL_RY,
            RelativeAxisType::REL_RZ,
            RelativeAxisType::REL_DIAL,
            RelativeAxisType::REL_MISC,
            RelativeAxisType::REL_WHEEL_HI_RES,
            RelativeAxisType::REL_HWHEEL_HI_RES,
        ]);

        let mut device = uinput::VirtualDeviceBuilder::new()?
            .name("kanata")
            .input_id(evdev::InputId::new(evdev::BusType::BUS_USB, 1, 1, 1))
            .with_keys(&keys)?
            .with_relative_axes(&relative_axes)?
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
            let osc = str_to_oscode(&s).expect("valid keycodes for unicode");
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

fn devices_from_input_paths(
    dev_paths: &[String],
    missing_device_paths: &mut Vec<String>,
) -> Vec<(Device, String)> {
    dev_paths
        .iter()
        .map(|dev_path| (dev_path, Device::open(dev_path)))
        .filter_map(|(dev_path, open_result)| match open_result {
            Ok(d) => Some((d, dev_path.clone())),
            Err(e) => {
                log::warn!("failed to open device '{dev_path}': {e:?}");
                missing_device_paths.push(dev_path.clone());
                None
            }
        })
        .collect()
}

fn discover_devices() -> Result<Vec<(Device, String)>, io::Error> {
    log::info!("looking for devices in /dev/input");
    let devices: Vec<_> = evdev::enumerate()
        .map(|(path, device)| {
            (
                device,
                path.to_str()
                    .expect("non-utf8 path found for device")
                    .to_owned(),
            )
        })
        .filter(|pd| is_input_device(&pd.0))
        .collect();
    if devices.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not auto detect any keyboard devices",
        ));
    }
    Ok(devices)
}

fn watch_devinput() -> Result<Inotify, io::Error> {
    let mut inotify = Inotify::init().expect("Failed to initialize inotify");
    inotify.add_watch("/dev/input", WatchMask::CREATE)?;
    Ok(inotify)
}

impl From<Btn> for OsCode {
    fn from(btn: Btn) -> Self {
        match btn {
            Btn::Left => OsCode::BTN_LEFT,
            Btn::Right => OsCode::BTN_RIGHT,
            Btn::Mid => OsCode::BTN_MIDDLE,
            Btn::Forward => OsCode::BTN_EXTRA,
            Btn::Backward => OsCode::BTN_SIDE,
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
            let mut signals = Signals::new([SIGINT, SIGTERM]).unwrap();
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

pub fn parse_dev_paths(paths: &str) -> Vec<String> {
    let mut all_paths = vec![];
    let mut full_dev_path = String::new();
    let mut dev_path_iter = paths.split(':').peekable();
    while let Some(dev_path) = dev_path_iter.next() {
        if dev_path.ends_with('\\') && dev_path_iter.peek().is_some() {
            full_dev_path.push_str(dev_path.trim_end_matches('\\'));
            full_dev_path.push(':');
            continue;
        } else {
            full_dev_path.push_str(dev_path);
        }
        all_paths.push(full_dev_path.clone());
        full_dev_path.clear();
    }
    all_paths
}

fn wait_for_all_keys_unpressed(dev: &Device) -> Result<(), io::Error> {
    let mut pending_release = false;
    const KEY_MAX: usize = OsCode::KEY_MAX as usize;
    let mut keystate = [0u8; KEY_MAX / 8 + 1];
    loop {
        let mut n_pressed_keys = 0;
        ioctl_read_buf!(read_keystates, 'E', 0x18, u8);
        unsafe { read_keystates(dev.as_raw_fd(), &mut keystate) }
            .map_err(|_| io::Error::last_os_error())?;
        for i in 0..=KEY_MAX {
            if (keystate[i / 8] >> (i % 8)) & 0x1 > 0 {
                n_pressed_keys += 1;
            }
        }
        match n_pressed_keys {
            0 => break,
            _ => pending_release = true,
        }
    }
    if pending_release {
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
    Ok(())
}

#[test]
fn test_parse_dev_paths() {
    assert_eq!(parse_dev_paths("h:w"), ["h", "w"]);
    assert_eq!(parse_dev_paths("h\\:w"), ["h:w"]);
    assert_eq!(parse_dev_paths("h\\:w\\"), ["h:w\\"]);
}

impl Drop for Symlink {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.dest);
        log::info!("Deleted symlink {:#?}", self.dest);
    }
}
