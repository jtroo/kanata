use anyhow::Result;

use parking_lot::Mutex;

use crate::kanata::*;

#[cfg(not(feature = "interception_driver"))]
mod llhook;

#[cfg(feature = "interception_driver")]
mod interception;

pub static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> =
    Lazy::new(|| Mutex::new(HashSet::default()));

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
            let keycode = match prev_state {
                State::NormalKey { keycode, coord, .. } => {
                    // Goal of this conditional:
                    //
                    // Do not process state if:
                    // - keycode is neither shift
                    // - keycode is at the position of either shift
                    // - state has not yet been released
                    if !matches!(keycode, KeyCode::LShift | KeyCode::RShift)
                        || *coord == (NORMAL_KEY_ROW, u16::from(OsCode::KEY_LEFTSHIFT))
                        || *coord == (NORMAL_KEY_ROW, u16::from(OsCode::KEY_RIGHTSHIFT))
                        || self
                            .layout
                            .bm()
                            .states
                            .iter()
                            .filter_map(state_filter)
                            .any(|s| s == *prev_state)
                    {
                        continue;
                    } else {
                        keycode
                    }
                }
                State::FakeKey { keycode } => {
                    // Goal of this conditional:
                    //
                    // Do not process state if:
                    // - keycode is neither shift
                    // - state has not yet been released
                    if !matches!(keycode, KeyCode::LShift | KeyCode::RShift)
                        || self
                            .layout
                            .bm()
                            .states
                            .iter()
                            .filter_map(state_filter)
                            .any(|s| s == *prev_state)
                    {
                        continue;
                    } else {
                        keycode
                    }
                }
                _ => continue,
            };
            log::debug!("lsft-arrowkey workaround: removing {keycode:?} at its typical coordinate");
            self.layout.bm().states.retain(|s| match s {
                State::LayerModifier { coord, .. }
                | State::Custom { coord, .. }
                | State::RepeatingSequence { coord, .. }
                | State::NormalKey { coord, .. } => {
                    *coord != (NORMAL_KEY_ROW, u16::from(OsCode::from(keycode)))
                }
                _ => true,
            });
            log::debug!("removing {keycode:?} from pressed keys");
            PRESSED_KEYS.lock().remove(&keycode.into());
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
