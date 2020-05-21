use crate::KbdIn;
use crate::KbdOut;
use crate::layers::LayersManager;

pub struct Ktrl {
    pub kbd_in: KbdIn,
    pub kbd_out: KbdOut,
    pub lmgr: LayersManager,
}

// use evdev_rs::enums::EventCode::EV_KEY;
// use evdev_rs::enums::EV_KEY::*;

impl Ktrl {
    pub fn new(kbd_in: KbdIn, kbd_out: KbdOut, lmgr: LayersManager) -> Self {
        return Self{kbd_in, kbd_out, lmgr}
    }

    pub fn event_loop(&mut self) -> Result<(), std::io::Error> {
        println!("Ktrl: Entering the event loop");
        loop {
            let in_event = self.kbd_in.read()?;
            dbg!(&in_event.event_code);
            self.kbd_out.write(&in_event)?;
        }
    }
}
