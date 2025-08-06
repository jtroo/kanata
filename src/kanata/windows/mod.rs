use anyhow::Result;

use parking_lot::Mutex;

use crate::kanata::*;

#[cfg(all(feature = "simulated_input", not(feature = "interception_driver")))]
mod exthook;
#[cfg(all(not(feature = "simulated_input"), feature = "interception_driver"))]
mod interception;
#[cfg(all(not(feature = "simulated_input"), not(feature = "interception_driver")))]
mod llhook;

pub static ALTGR_BEHAVIOUR: Lazy<Mutex<AltGrBehaviour>> =
    Lazy::new(|| Mutex::new(AltGrBehaviour::default()));

pub fn set_win_altgr_behaviour(b: AltGrBehaviour) {
    *ALTGR_BEHAVIOUR.lock() = b;
}

impl Kanata {
    #[cfg(all(
        not(feature = "interception_driver"),
        not(feature = "simulated_output"),
        not(feature = "win_sendinput_send_scancodes"),
    ))]
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

    #[cfg(any(
        feature = "interception_driver",
        feature = "simulated_output",
        feature = "win_sendinput_send_scancodes"
    ))]
    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }

    #[cfg(feature = "gui")]
    pub fn live_reload(&mut self) -> Result<()> {
        self.live_reload_requested = true;
        self.do_live_reload(&None)?;
        Ok(())
    }
    #[cfg(feature = "gui")]
    pub fn live_reload_n(&mut self, n: usize) -> Result<()> {
        // can't use in CustomAction::LiveReloadNum(n) due to 2nd mut borrow
        self.live_reload_requested = true;
        // let backup_cfg_idx = self.cur_cfg_idx;
        match self.cfg_paths.get(n) {
            Some(path) => {
                self.cur_cfg_idx = n;
                log::info!("Requested live reload of file: {}", path.display(),);
            }
            None => {
                log::error!(
                    "Requested live reload of config file number {}, but only {} config files were passed",
                    n + 1,
                    self.cfg_paths.len()
                );
            }
        }
        // if let Err(e) = self.do_live_reload(&None) {
        // self.cur_cfg_idx = backup_cfg_idx; // restore index on fail when. TODO: add when a similar reversion is added to other custom actions
        // return Err(e)
        // }
        self.do_live_reload(&None)?;
        Ok(())
    }
}

/// If kanata has been inactive for long enough, clear all states.
/// This won't trigger if there are macros running, or if a key is
/// held down for a long time and is sending OS repeats. The reason
/// for this code is in case like Win+L which locks the Windows
/// desktop. When this happens, the Win key and L key will be stuck
/// as pressed in the kanata state because LLHOOK kanata cannot read
/// keys in the lock screen or administrator applications. So this
/// is heuristic to detect such an issue and clear states assuming
/// that's what happened.
///
/// Only states in the normal key row are cleared, since those are
/// the states that might be stuck. A real use case might be to have
/// a fake key pressed for a long period of time, so make sure those
/// are not cleared.
#[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
pub fn clear_states_from_inactivity(
    k: &mut parking_lot::MutexGuard<Kanata>,
    now: web_time::Instant,
    last_input_time: web_time::Instant,
    idle_clear_happened: &mut bool,
) {
    if (now - (last_input_time)) > time::Duration::from_secs(LLHOOK_IDLE_TIME_SECS_CLEAR_INPUTS)
        && !*idle_clear_happened
    {
        *idle_clear_happened = true;
        log::debug!("clearing keyberon normal key states due to inactivity");
        let layout = k.layout.bm();
        release_normalkey_states(layout);
        PRESSED_KEYS.lock().clear();
    }
}
