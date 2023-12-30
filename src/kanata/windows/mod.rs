use anyhow::Result;

use parking_lot::Mutex;

use crate::kanata::*;

#[cfg(not(feature = "interception_driver"))]
mod llhook;

#[cfg(feature = "interception_driver")]
mod interception;

static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> = Lazy::new(|| Mutex::new(HashSet::default()));

pub static ALTGR_BEHAVIOUR: Lazy<Mutex<AltGrBehaviour>> =
    Lazy::new(|| Mutex::new(AltGrBehaviour::default()));

pub fn set_win_altgr_behaviour(b: AltGrBehaviour) {
    *ALTGR_BEHAVIOUR.lock() = b;
}

impl Kanata {
    #[cfg(not(feature = "interception_driver"))]
    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        fn state_filter(v: &State<'_, &&[&CustomAction]>) -> Option<State<'static, ()>> {
            match v {
                State::NormalKey {
                    keycode,
                    coord,
                    flags,
                } => Some(State::NormalKey::<()> {
                    keycode: *keycode,
                    coord: *coord,
                    flags: *flags,
                }),
                State::FakeKey { keycode } => Some(State::FakeKey::<()> { keycode: *keycode }),
                _ => None,
            }
        }

        static PREV_STATES: Lazy<Mutex<Vec<State<'static, ()>>>> = Lazy::new(|| Mutex::new(vec![]));
        let mut prev_states = PREV_STATES.lock();

        if prev_states.is_empty() {
            prev_states.extend(
                self.layout
                    .bm()
                    .states
                    .as_slice()
                    .iter()
                    .filter_map(state_filter),
            );
            return Ok(());
        }

        // This is an n^2 loop, but realistically there should be <= 5 states at a given time so
        // this should not be a problem. State does not implement Hash so can't use a HashSet. A
        // HashSet might perform worse anyway.
        for prev_state in prev_states.iter() {
            if let State::NormalKey { keycode, coord, .. } = prev_state {
                if !matches!(keycode, KeyCode::LShift | KeyCode::RShift)
                    || (matches!(keycode, KeyCode::LShift)
                        && coord.1 == u16::from(OsCode::KEY_LEFTSHIFT))
                    || (matches!(keycode, KeyCode::RShift)
                        && coord.1 == u16::from(OsCode::KEY_RIGHTSHIFT))
                    || self
                        .layout
                        .bm()
                        .states
                        .iter()
                        .filter_map(state_filter)
                        .any(|s| s == *prev_state)
                {
                    continue;
                }
                log::debug!(
                    "lsft-arrowkey workaround: releasing {keycode:?} at its typical coordinate"
                );
                self.layout.bm().states.retain(|s| match s {
                    State::NormalKey {
                        keycode: cur_kc,
                        coord: cur_coord,
                        ..
                    } => cur_kc != keycode && *cur_coord != (0, u16::from(OsCode::from(keycode))),
                    _ => true,
                });
                log::debug!("releasing {keycode:?} from pressed keys");
                PRESSED_KEYS.lock().remove(&keycode.into());
                if let Err(e) = self.kbd_out.release_key(keycode.into()) {
                    bail!("failed to release key: {:?}", e);
                }
            }
        }

        prev_states.clear();
        prev_states.extend(self.layout.bm().states.iter().filter_map(state_filter));
        Ok(())
    }

    #[cfg(feature = "interception_driver")]
    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }
}
