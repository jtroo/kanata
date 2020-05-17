use evdev_rs::ReadFlag;
use evdev_rs::Device;
use evdev_rs::UInputDevice;

pub fn ktrl_process(kbd_in: Device, kbd_out: ()) -> Result<(), std::io::Error> {
    loop {
        let a = kbd_in.next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING);
        match a {
            Ok(k) => println!("Event: time {}.{}, ++++++++++++++++++++ {} +++++++++++++++",
                              k.1.time.tv_sec,
                              k.1.time.tv_usec,
                              k.1.event_type),
            Err(e) => (),
        }
    }

    Ok(())
}
