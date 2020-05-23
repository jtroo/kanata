use evdev_rs::enums::EventType;
use log::info;

use crate::KbdIn;
use crate::KbdOut;
use std::convert::TryFrom;
use crate::keys::KeyEvent;
use crate::layers::LayersManager;
use crate::actions::TapHoldMgr;
use crate::effects::key_event_to_fx_val;
use crate::effects::perform_effect;
use crate::effects::StickyState;

pub struct Ktrl {
    pub kbd_in: KbdIn,
    pub kbd_out: KbdOut,
    pub l_mgr: LayersManager,
    pub th_mgr: TapHoldMgr,
    pub sticky: StickyState,
}

impl Ktrl {
    fn handle_key_event(&mut self, event: &KeyEvent) -> Result<(), std::io::Error> {
        // Handle TapHold action keys
        let th_out = self.th_mgr.process(&mut self.l_mgr, event);
        if let Some(th_fx_vals) = th_out.effects {
            for fx_val in th_fx_vals {
                perform_effect(self, fx_val)?
            }
        }

        // Handle leftover effect(s)
        if !th_out.stop_processing {
            let leftover_fx_val = key_event_to_fx_val(&self.l_mgr, event);
            perform_effect(self, leftover_fx_val)?;
        }

        Ok(())
    }

    pub fn event_loop(&mut self) -> Result<(), std::io::Error> {
        info!("Ktrl: Entering the event loop");

        loop {
            let in_event = self.kbd_in.read()?;

            // Filter uninteresting events
            if in_event.event_type == EventType::EV_SYN ||
                in_event.event_type == EventType::EV_MSC {
                continue;
            }

            // Pass-through non-key events
            let key_event = match KeyEvent::try_from(in_event.clone()) {
                Ok(ev) => ev,
                _ =>  {
                    self.kbd_out.write(in_event)?;
                    continue;
                }
            };

            self.handle_key_event(&key_event)?;
        }
    }
}
