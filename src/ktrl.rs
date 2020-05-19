use crate::KbdIn;
use crate::KbdOut;

pub struct Ktrl {
    kbd_in: KbdIn,
    kbd_out: KbdOut,
}

// REMOVE
use evdev_rs::enums::EventCode::EV_KEY;
use evdev_rs::enums::EV_KEY::*;

impl Ktrl {
    pub fn new(kbd_in: KbdIn, kbd_out: KbdOut) -> Self {
        return Self{kbd_in, kbd_out}
    }

    pub fn event_loop(&mut self) -> Result<(), std::io::Error> {
        println!("Ktrl: Entering the event loop");
        loop {
            let in_event = self.kbd_in.read()?;
            dbg!(&in_event.event_code);
            if let EV_KEY(KEY_A) = in_event.event_code {
                self.kbd_out.tap_key(KEY_B)?;
            }
            self.kbd_out.write(&in_event)?;
        }
    }
}
