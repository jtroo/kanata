use super::*;

const RELEASE_AFTER_MS: u16 = 5;

#[derive(Debug, PartialEq)]
enum RepeatPhase {
    /// Key was just pressed. Wait a few ms then release it from the HID
    /// report to prevent macOS OS-level repeat from firing.
    HeldBeforeRelease { ticks_remaining: u16 },
    /// Key released from HID report. Wait until managed repeat delay elapses.
    ReleasedWaiting { ticks_remaining: u16 },
    /// Actively repeating via release+re-press cycles.
    Repeating { ticks_remaining: u16 },
}

#[derive(Debug)]
struct RepeatTimer {
    osc: OsCode,
    delay: u16,
    interval: u16,
    phase: RepeatPhase,
}

#[derive(Debug)]
pub struct ManagedRepeatState {
    timers: HashMap<OsCode, RepeatTimer>,
    overrides: HashMap<OsCode, (u16, u16)>,
    default_delay: u16,
    default_interval: u16,
}

impl ManagedRepeatState {
    pub fn new(default_delay: u16, default_interval: u16) -> Self {
        Self {
            timers: HashMap::default(),
            overrides: HashMap::default(),
            default_delay,
            default_interval,
        }
    }

    pub fn add_override(&mut self, osc: OsCode, delay: u16, interval: u16) {
        self.overrides.insert(osc, (delay, interval));
    }

    fn timing_for(&self, osc: OsCode) -> (u16, u16) {
        self.overrides
            .get(&osc)
            .copied()
            .unwrap_or((self.default_delay, self.default_interval))
    }

    pub fn is_idle(&self) -> bool {
        self.timers.is_empty()
    }
}

impl Kanata {
    pub(super) fn tick_managed_repeat(&mut self) -> Result<()> {
        let state = match self.managed_repeat_state.as_mut() {
            Some(s) => s,
            None => return Ok(()),
        };

        // Remove timers for keys no longer in cur_keys (physically released).
        state.timers.retain(|_osc, timer| {
            let kc: KeyCode = timer.osc.into();
            self.cur_keys.contains(&kc)
        });

        // Start timers for newly pressed non-modifier keys.
        for k in self.cur_keys.iter() {
            let osc: OsCode = (*k).into();
            if osc.is_modifier() {
                continue;
            }
            if state.timers.contains_key(&osc) {
                continue;
            }
            let (delay, interval) = state.timing_for(osc);
            state.timers.insert(
                osc,
                RepeatTimer {
                    osc,
                    delay,
                    interval,
                    phase: RepeatPhase::HeldBeforeRelease {
                        ticks_remaining: RELEASE_AFTER_MS,
                    },
                },
            );
        }

        // Tick all timers and collect actions.
        let mut releases = Vec::new();
        let mut presses = Vec::new();

        for timer in state.timers.values_mut() {
            match &mut timer.phase {
                RepeatPhase::HeldBeforeRelease { ticks_remaining } => {
                    *ticks_remaining = ticks_remaining.saturating_sub(1);
                    if *ticks_remaining == 0 {
                        releases.push(timer.osc);
                        let wait = timer.delay.saturating_sub(RELEASE_AFTER_MS);
                        timer.phase = RepeatPhase::ReleasedWaiting {
                            ticks_remaining: wait,
                        };
                    }
                }
                RepeatPhase::ReleasedWaiting { ticks_remaining } => {
                    *ticks_remaining = ticks_remaining.saturating_sub(1);
                    if *ticks_remaining == 0 {
                        presses.push(timer.osc);
                        timer.phase = RepeatPhase::Repeating {
                            ticks_remaining: timer.interval,
                        };
                    }
                }
                RepeatPhase::Repeating { ticks_remaining } => {
                    *ticks_remaining = ticks_remaining.saturating_sub(1);
                    if *ticks_remaining == 0 {
                        releases.push(timer.osc);
                        presses.push(timer.osc);
                        *ticks_remaining = timer.interval;
                    }
                }
            }
        }

        for osc in &releases {
            if let Err(e) = release_key(&mut self.kbd_out, *osc) {
                bail!("managed repeat release failed: {e:?}");
            }
        }
        for osc in &presses {
            log::info!("managed repeat {:?}", KeyCode::from(*osc));
            if let Err(e) = press_key(&mut self.kbd_out, *osc) {
                bail!("managed repeat press failed: {e:?}");
            }
        }

        Ok(())
    }
}
