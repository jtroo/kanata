use super::*;

#[derive(Debug)]
struct RepeatTimer {
    osc: OsCode,
    ticks_remaining: u16,
    interval: u16,
    repeating: bool,
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

        // Remove timers for keys no longer in cur_keys (just released).
        state.timers.retain(|_osc, timer| {
            let kc: KeyCode = timer.osc.into();
            self.cur_keys.contains(&kc)
        });

        // Start timers for newly pressed non-modifier keys.
        // We check if a timer already exists rather than comparing prev_keys,
        // because handle_keystate_changes pushes new keys into prev_keys before
        // this function runs.
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
                    ticks_remaining: delay,
                    interval,
                    repeating: false,
                },
            );
        }

        // Tick all timers and collect keys that need a repeat event.
        let mut repeats = Vec::new();
        for timer in state.timers.values_mut() {
            timer.ticks_remaining = timer.ticks_remaining.saturating_sub(1);
            if timer.ticks_remaining == 0 {
                repeats.push(timer.osc);
                timer.ticks_remaining = timer.interval;
                timer.repeating = true;
            }
        }

        for osc in repeats {
            log::info!("managed repeat {:?}", KeyCode::from(osc));
            if let Err(e) = write_key(&mut self.kbd_out, osc, KeyValue::Repeat) {
                bail!("managed repeat failed: {e:?}");
            }
        }

        Ok(())
    }
}
