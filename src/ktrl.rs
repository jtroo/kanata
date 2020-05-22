use evdev_rs::InputEvent;
use evdev_rs::enums::EventCode;
use evdev_rs::enums::EventType;
use inner::*;

use crate::KbdIn;
use crate::KbdOut;
use crate::layers::LayersManager;
use crate::actions::TapHoldMgr;

// TODO: TapHold handling, move this...
use crate::layers::Effect;
use crate::keyevent::KeyValue;
use crate::actions::tap_hold::TapHoldEffect;

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

            let th_out = self.th_mgr.process_tap_hold(&mut self.l_mgr, &in_event);
            if let Some(th_effects) = th_out.effects {
                for fx in th_effects {
                    match fx {
                        TapHoldEffect{fx: Effect::Default(kc), val: KeyValue::Press} => {
                            self.kbd_out.press_key(kc.into())?;
                        },
                        TapHoldEffect{fx: Effect::Default(kc), val: KeyValue::Release} => {
                            self.kbd_out.release_key(kc.into())?;
                        },
                        _ => assert!(false),
                    }
                }
            }

            if !th_out.stop_processing {
                match in_event {
                    InputEvent{event_code: EventCode::EV_KEY(evkey), ..} => self.kbd_out.write_key(evkey, in_event.value)?,
                    ev => self.kbd_out.write(&ev)?,
                }
            }
        }
    }
}
