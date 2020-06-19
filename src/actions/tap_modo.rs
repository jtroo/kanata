use crate::actions::Action;
use crate::effects::Effect;
use crate::effects::OutEffects;
use crate::effects::{CONTINUE, STOP};
use crate::keys::KeyCode;
use crate::keys::KeyEvent;
use crate::keys::KeyValue;
use crate::layers::LayersManager;

pub struct TapModoMgr {
    modifiers: Vec<bool>,
}

fn is_shift(kc: KeyCode) -> bool {
    kc == KeyCode::KEY_LEFTSHIFT || kc == KeyCode::KEY_RIGHTSHIFT
}

impl TapModoMgr {
    pub fn new() -> Self {
        let modifiers = Vec::new();
        modifiers.resize_with(KeyCode::KEY_MAX as usize, || false);
        Self { modifiers }
    }

    fn process_tap_modo(&self, event: &KeyEvent) -> OutEffects {
        let effect = {
            if self.is_shift_on {
                Effect::Key(KeyCode::KEY_GRAVE)
            } else {
                Effect::Key(KeyCode::KEY_ESC)
            }
        };

        OutEffects::new(STOP, effect, event.value)
    }

    fn process_non_tap_modo(&mut self, event: &KeyEvent) -> OutEffects {
        if is_shift(event.code) {
            match event.value {
                KeyValue::Press => self.is_shift_on = true,
                KeyValue::Repeat => self.is_shift_on = true,
                KeyValue::Release => self.is_shift_on = false,
            }
        }

        OutEffects::empty(CONTINUE)
    }

    pub fn process(&mut self, l_mgr: &LayersManager, event: &KeyEvent) -> OutEffects {
        let code = event.code;
        let action = &l_mgr.get(code).action;

        if let Action::TapModo = action {
            self.process_tap_modo(event)
        } else {
            self.process_non_tap_modo(event)
        }
    }
}
