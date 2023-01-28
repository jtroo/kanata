use anyhow::{bail, Result};

use parking_lot::Mutex;

use crate::cfg;
use crate::kanata::*;

#[cfg(not(feature = "interception_driver"))]
mod llhook;
#[cfg(not(feature = "interception_driver"))]
pub use llhook::*;

#[cfg(feature = "interception_driver")]
mod interception;
#[cfg(feature = "interception_driver")]
pub use self::interception::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltGrBehaviour {
    DoNothing,
    CancelLctlPress,
    AddLctlRelease,
}

pub static ALTGR_BEHAVIOUR: Lazy<Mutex<AltGrBehaviour>> =
    Lazy::new(|| Mutex::new(AltGrBehaviour::DoNothing));

pub fn set_win_altgr_behaviour(cfg: &cfg::Cfg) -> Result<()> {
    *ALTGR_BEHAVIOUR.lock() = {
        const CANCEL: &str = "cancel-lctl-press";
        const ADD: &str = "add-lctl-release";
        match cfg.items.get("windows-altgr") {
            None => AltGrBehaviour::DoNothing,
            Some(cfg_val) => match cfg_val.as_str() {
                CANCEL => AltGrBehaviour::CancelLctlPress,
                ADD => AltGrBehaviour::AddLctlRelease,
                _ => bail!(
                    "Invalid value for windows-altgr: {}. Valid values are {},{}",
                    cfg_val,
                    CANCEL,
                    ADD
                ),
            },
        }
    };
    Ok(())
}

fn state_filter(v: &State<'_, &[&CustomAction]>) -> Option<State<'static, ()>> {
    match v {
        State::NormalKey { keycode, coord } => Some(State::NormalKey::<()> {
            keycode: *keycode,
            coord: *coord,
        }),
        State::FakeKey { keycode } => Some(State::FakeKey::<()> { keycode: *keycode }),
        _ => None,
    }
}

impl Kanata {
    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
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
            if let State::NormalKey { keycode, coord } = prev_state {
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
                log::debug!("releasing all {keycode:?}");
                self.layout.bm().states.retain(|s| match s {
                    State::NormalKey {
                        keycode: cur_kc, ..
                    }
                    | State::FakeKey { keycode: cur_kc } => cur_kc != keycode,
                    _ => true,
                });
                if let Err(e) = self.kbd_out.release_key(keycode.into()) {
                    bail!("failed to release key: {:?}", e);
                }
            }
        }

        prev_states.clear();
        prev_states.extend(self.layout.bm().states.iter().filter_map(state_filter));
        Ok(())
    }
}
