// evdev-rs
use evdev_rs::Device;
use evdev_rs::UInputDevice;
use evdev_rs::GrabMode;

// uinput-rs
use uinput_sys;
use uinput_sys::uinput_user_dev;

// std
use std::fs::OpenOptions;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;

// pub fn ktrl_setup(kbd_path: &Path) -> Result<(Device, UInputDevice), std::io::Error> {
pub fn ktrl_setup(kbd_path: &Path) -> Result<(Device, ()), std::io::Error> {
    println!("0");
    let kbd_in_file = File::open(kbd_path)?;
    let mut kbd_in_dev = Device::new_from_fd(kbd_in_file)?;
    kbd_in_dev.grab(GrabMode::Grab)?;
    println!("1");

    let mut uinput_out_file = OpenOptions::new().read(true).write(true).open("/dev/uinput")?;
    // let _kbd_out_dev = Device::new_from_fd(_uinput_out_file)?;
    // let kbd_out_dev = UInputDevice::create_from_device(&_kbd_out_dev)?;
    // let mut uinput_out_file = kbd_out_dev.fd().unwrap();

    unsafe {
        println!("2");
        uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_SYN);
        println!("3");
        uinput_sys::ui_set_evbit(uinput_out_file.as_raw_fd(), uinput_sys::EV_KEY);
        println!("4");

        for key in 0..uinput_sys::KEY_MAX {
            uinput_sys::ui_set_keybit(uinput_out_file.as_raw_fd(), key);
        }

        println!("5");
        let mut uidev: uinput_user_dev = std::mem::zeroed();
        uidev.name[0] = 'k' as i8;
        uidev.name[1] = 't' as i8;
        uidev.name[2] = 'r' as i8;
        uidev.name[3] = 'l' as i8;
        uidev.id.bustype = 0x3; // BUS_USB
        uidev.id.vendor  = 0x1;
        uidev.id.product = 0x1;
        uidev.id.version = 1;

        let cstr_uidev = std::ffi::CStr::from_ptr(std::mem::transmute(&uidev));
        println!("6");
        uinput_out_file.write(cstr_uidev.to_bytes())?;
        println!("7");
        uinput_sys::ui_dev_create(uinput_out_file.as_raw_fd());
        println!("8");
    }

    // Ok((kbd_in_dev, kbd_out_dev))
    Ok((kbd_in_dev, ()))
}
