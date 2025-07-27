use std::time::{Duration, Instant};
use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;

use crate::kanata::{KeyEvent, KeyValue};
use std::sync::mpsc::SyncSender as Sender;
use crate::kanata::debounce::debounce::{try_send_panic, Debounce};

/// Implementation of the sym_defer_pk algorithm
/// Debouncing per key. On any state change, a per-key timer is set.
/// When DEBOUNCE milliseconds of no changes have occurred on that key,
/// the key status change is pushed.
pub struct SymDeferPk {
    debounce_duration: Duration,
    pending_events: Vec<(KeyEvent, Instant)>,
}

impl SymDeferPk {
    pub fn new(debounce_duration_ms: u16) -> Self {
        Self {
            debounce_duration: Duration::from_millis(debounce_duration_ms.into()),
            pending_events: Vec::new(),
        }
    }
}

impl Debounce for SymDeferPk {
    fn name(&self) -> DebounceAlgorithm {
        DebounceAlgorithm::SymDeferPk
    }

    fn debounce_time(&self) -> u16 {
        self.debounce_duration.as_millis() as u16
    }

    fn process_event(&mut self, event: KeyEvent, process_tx: &Sender<KeyEvent>) -> bool {
        let now = Instant::now();
        let oscode = event.code;

        match event.value {
            KeyValue::Repeat => {
                // Forward repeat events immediately
                log::debug!("Forwarding repeat event for {:?}", oscode);
                try_send_panic(process_tx, event);
            }
            _ => {
                let new_deadline = now + self.debounce_duration;

                // Check if there is a pending event for this key
                if let Some(pos) = self.pending_events.iter().position(|(pending_event, _)| pending_event.code == oscode) {
                    // If the event is already pending, update the deadline
                    log::debug!(
                        "Updating pending event for {:?} (value: {:?}) to new deadline: {:?}",
                        oscode,
                        event.value,
                        new_deadline
                    );
                    self.pending_events[pos].1 = new_deadline;
                } else {
                    // No pending event for this key. Add the new event.
                    log::debug!(
                        "Deferring event for {:?} (value: {:?}) until debounce duration passes",
                        oscode, event.value
                    );
                    self.pending_events.push((event, new_deadline));
                }
            }
        }

        // Return true if there are still pending events
        !self.pending_events.is_empty()
    }

    fn tick(&mut self, process_tx: &Sender<KeyEvent>, now: Instant) -> bool {
        // Process any events whose debounce duration has passed
        self.pending_events.retain(|(event, deadline)| {
            if now >= *deadline {
                log::debug!("Emitting deferred event for {:?}", event.code);
                try_send_panic(process_tx, event.clone());
                false // Remove this item from the Vec
            } else {
                true // Keep this item in the Vec
            }
        });

        // Return true if there are still pending events
        !self.pending_events.is_empty()
    }
}
