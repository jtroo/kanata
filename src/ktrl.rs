use evdev_rs::enums::EventType;

use crate::KbdIn;
use crate::KbdOut;
use crate::layers::LayersManager;
use crate::actions::TapHoldMgr;
use crate::effects::event_to_default_fx_val;
use crate::effects::perform_effect;

pub struct Ktrl {
    pub kbd_in: KbdIn,
    pub kbd_out: KbdOut,
    pub l_mgr: LayersManager,
    pub th_mgr: TapHoldMgr,
}

impl Ktrl {
    pub fn new(kbd_in: KbdIn, kbd_out: KbdOut, l_mgr: LayersManager, th_mgr: TapHoldMgr) -> Self {
        return Self{kbd_in, kbd_out, l_mgr, th_mgr}
    }

    pub fn event_loop(&mut self) -> Result<(), std::io::Error> {
        println!("Ktrl: Entering the event loop");

        loop {
            let in_event = self.kbd_in.read()?;

            // Filter uninteresting events
            if in_event.event_type == EventType::EV_SYN ||
                in_event.event_type == EventType::EV_MSC {
                continue;
            }

            let th_out = self.th_mgr.process(&mut self.l_mgr, &in_event);
            if let Some(th_fx_vals) = th_out.effects {
                for fx_val in th_fx_vals {
                    perform_effect(&mut self.kbd_out, fx_val)?
                }
            }

            if !th_out.stop_processing {
                if let Some(leftover_fx_val) = event_to_default_fx_val(&in_event) {
                    perform_effect(&mut self.kbd_out, leftover_fx_val)?;
                } else {
                    self.kbd_out.write(&in_event)?;
                }
            }
        }
    }
}
