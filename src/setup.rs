// uinput-rs
use uinput_sys;
use uinput_sys::uinput_user_dev;

// std
use std::fs::OpenOptions;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;

pub struct UInputDevice {
    file: std::fs::File,
}

pub fn ktrl_setup(kbd_path: &Path) -> Result<(Device, UInputDevice), std::io::Error> {
    let kbd_in_file = File::open(kbd_path)?;
    let mut kbd_in_dev = Device::new_from_fd(kbd_in_file)?;
    kbd_in_dev.grab(GrabMode::Grab)?;

    let mut uinput_out_file = OpenOptions::new().read(true).write(true).open("/dev/uinput")?;
    // let _kbd_out_dev = Device::new_from_fd(_uinput_out_file)?;
    // let kbd_out_dev = UInputDevice::create_from_device(&_kbd_out_dev)?;
    // let mut uinput_out_file = kbd_out_dev.fd().unwrap();

    unsafe {
        uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_SYN);
        uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_KEY);

        for key in 0..uinput_sys::KEY_MAX {
            uinput_sys::ui_set_keybit(uinput_out_file.as_raw_fd(), key);
        }

        let mut uidev: uinput_user_dev = std::mem::zeroed();
        uidev.name[0] = 'k' as i8;
        uidev.name[1] = 't' as i8;
        uidev.name[2] = 'r' as i8;
        uidev.name[3] = 'l' as i8;
        uidev.id.bustype = 0x3; // BUS_USB
        uidev.id.vendor  = 0x1;
        uidev.id.product = 0x1;
        uidev.id.version = 1;

        let uidev_bytes: &[u8] = std::slice::from_raw_parts(std::mem::transmute(&uidev), std::mem::size_of::<uinput_user_dev>());
        uinput_out_file.write(uidev_bytes)?;
        uinput_sys::ui_dev_create(uinput_out_file.as_raw_fd());
    }

    // Ok((kbd_in_dev, kbd_out_dev))
    Ok((kbd_in_dev, UInputDevice{file: uinput_out_file}))
}
