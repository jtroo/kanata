use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
use std::time::{Duration, Instant};

use crate::kanata::KeyEvent;
use crate::kanata::debounce::debounce::{Debounce, try_send_panic};
use crate::oskbd::KeyValue;
use std::sync::mpsc::SyncSender as Sender;

/// Implementation of the sym_eager_pk algorithm
/// See: https://github.com/qmk/qmk_firmware/blob/6ef97172889ccd5db376b2a9f8825489e24fdac4/docs/feature_debounce_type.md
/// Debouncing per key. On any state change, response is immediate,
/// followed by debounce_duration milliseconds of no further input for that key.
pub struct SymEagerPk {
    debounce_duration: Duration,
    /// Keys whose last state change was sent less than `debounce_duration` ago.
    lockouts: Vec<Lockout>,
}

struct Lockout {
    /// The event that was sent.
    sent: KeyEvent,
    /// The latest event seen for the key since then.
    latest: KeyEvent,
    until: Instant,
}

impl SymEagerPk {
    pub fn new(debounce_duration_ms: u16) -> Self {
        Self {
            debounce_duration: Duration::from_millis(debounce_duration_ms.into()),
            lockouts: Vec::new(),
        }
    }
}

impl Debounce for SymEagerPk {
    fn name(&self) -> DebounceAlgorithm {
        DebounceAlgorithm::SymEagerPk
    }

    fn debounce_time(&self) -> u16 {
        self.debounce_duration.as_millis() as u16
    }

    fn process_event(
        &mut self,
        event: KeyEvent,
        process_tx: &Sender<KeyEvent>,
        now: Instant,
    ) -> bool {
        let oscode = event.code;

        // If event is a repeat, process it immediately
        if event.value == KeyValue::Repeat {
            log::debug!(
                "Processing repeat event for {:?}: {:?}",
                oscode,
                event.value
            );
            try_send_panic(process_tx, event);
            return !self.lockouts.is_empty();
        }

        // End any lockouts that are over before looking at this key.
        self.tick(process_tx, now);

        // Within the lockout, only remember the latest state.
        if let Some(lockout) = self.lockouts.iter_mut().find(|l| l.sent.code == oscode) {
            log::debug!("Debouncing event for {:?}: {:?}", oscode, event.value);
            lockout.latest = event;
            return true;
        }

        // Process the event immediately
        log::debug!("Processing event for {:?}: {:?}", oscode, event.value);
        try_send_panic(process_tx, event);
        self.lockouts.push(Lockout {
            sent: event,
            latest: event,
            until: now + self.debounce_duration,
        });
        true
    }

    fn tick(&mut self, process_tx: &Sender<KeyEvent>, now: Instant) -> bool {
        let debounce_duration = self.debounce_duration;
        self.lockouts.retain_mut(|lockout| {
            if now < lockout.until {
                return true;
            }
            if lockout.latest.value == lockout.sent.value {
                // The key ended up where it was: nothing changed.
                return false;
            }
            // The key changed state during the lockout: send that, and lock it again.
            log::debug!(
                "Emitting state change for {:?}: {:?}",
                lockout.latest.code,
                lockout.latest.value
            );
            try_send_panic(process_tx, lockout.latest);
            lockout.sent = lockout.latest;
            lockout.until = now + debounce_duration;
            true
        });
        !self.lockouts.is_empty()
    }
}
