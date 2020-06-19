use crate::actions::Action;
use crate::effects::Effect;
use crate::effects::Effect::*;
use crate::effects::EffectValue;
use crate::effects::OutEffects;
use crate::effects::{CONTINUE, STOP};
use crate::keys::KeyCode;
use crate::keys::KeyCode::*;
use crate::keys::KeyEvent;
use crate::keys::KeyValue;
use crate::layers::LayersManager;

pub struct TapModMgr {
    modifiers: Vec<bool>,
}

impl TapModMgr {
    pub fn new() -> Self {
        let mut modifiers = Vec::new();
        modifiers.resize_with(KeyCode::KEY_MAX as usize, || false);
        Self { modifiers }
    }

    fn process_tap_mod(
        &self,
        event: &KeyEvent,
        modifier: KeyCode,
        tap_fx: &Effect,
        mod_fx: &Effect,
        is_modo: bool,
    ) -> OutEffects {
        let mod_state = self.modifiers[modifier as usize];
        let fx_vals = {
            match (mod_state, is_modo) {
                (true, true) => vec![
                    EffectValue::new(Key(modifier), KeyValue::Release),
                    EffectValue::new(mod_fx.clone(), event.value),
                    EffectValue::new(Key(modifier), KeyValue::Press),
                ],
                (true, false) => vec![EffectValue::new(mod_fx.clone(), event.value)],
                (false, _) => vec![EffectValue::new(tap_fx.clone(), event.value)],
            }
        };

        dbg!(OutEffects::new_multiple(STOP, fx_vals))
    }

    fn process_non_tap_mod(&mut self, event: &KeyEvent) -> OutEffects {
        let mod_state = &mut self.modifiers[event.code as usize];
        match event.value {
            KeyValue::Press => *mod_state = true,
            KeyValue::Repeat => *mod_state = true,
            KeyValue::Release => *mod_state = false,
        }

        OutEffects::empty(CONTINUE)
    }

    fn get_action(l_mgr: &LayersManager, event: &KeyEvent) -> Action {
        let code = event.code;
        let action = &l_mgr.get(code).action;

        match action {
            Action::TildeEsc => Action::TapModi(KEY_LEFTSHIFT, Key(KEY_ESC), Key(KEY_GRAVE)),
            _ => action.clone(),
        }
    }

    pub fn process(&mut self, l_mgr: &LayersManager, event: &KeyEvent) -> OutEffects {
        let action = Self::get_action(l_mgr, event);
        match action {
            Action::TapModi(modifier, tap_fx, mod_fx) => {
                self.process_tap_mod(event, modifier, &tap_fx, &mod_fx, false)
            }
            Action::TapModo(modifier, tap_fx, mod_fx) => {
                self.process_tap_mod(event, modifier, &tap_fx, &mod_fx, true)
            }
            _ => self.process_non_tap_mod(event),
        }
    }
}
