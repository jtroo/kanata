// evdev-rs
use evdev_rs::Device;
use evdev_rs::GrabMode;

use std::fs::File;
use std::path::Path;

pub struct KbdIn {
    device: Device,
}

impl KbdIn {
    fn new(dev_path: &Path) -> Result<Self, std::io::Error> {
        let kbd_in_file = File::open(dev_path)?;
        let mut kbd_in_dev = Device::new_from_fd(kbd_in_file)?;
        kbd_in_dev.grab(GrabMode::Grab)?;
        Ok(KbdIn {device: kbd_in_dev})
    }
}
