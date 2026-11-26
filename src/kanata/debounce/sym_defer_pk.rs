use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::kanata::debounce::debounce::{Debounce, try_send_panic};
use crate::kanata::{KeyEvent, KeyValue, OsCode};
use std::sync::mpsc::SyncSender as Sender;

/// Implementation of the sym_defer_pk algorithm
/// Debouncing per key. On any state change, a per-key timer is set.
/// When DEBOUNCE milliseconds of no changes have occurred on that key,
/// the key status change is pushed.
pub struct SymDeferPk {
    debounce_duration: Duration,
    pending_events: Vec<(KeyEvent, Instant)>,
    /// The last value sent for each key.
    sent_values: HashMap<OsCode, KeyValue>,
}

impl SymDeferPk {
    pub fn new(debounce_duration_ms: u16) -> Self {
        Self {
            debounce_duration: Duration::from_millis(debounce_duration_ms.into()),
            pending_events: Vec::new(),
            sent_values: HashMap::new(),
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
                if let Some(pos) = self
                    .pending_events
                    .iter()
                    .position(|(pending_event, _)| pending_event.code == oscode)
                {
                    // Keep the latest state and restart the wait for it to settle
                    log::debug!(
                        "Updating pending event for {:?} (value: {:?}) to new deadline: {:?}",
                        oscode,
                        event.value,
                        new_deadline
                    );
                    self.pending_events[pos] = (event, new_deadline);
                } else {
                    // No pending event for this key. Add the new event.
                    log::debug!(
                        "Deferring event for {:?} (value: {:?}) until debounce duration passes",
                        oscode,
                        event.value
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
        let sent_values = &mut self.sent_values;
        self.pending_events.retain(|(event, deadline)| {
            if now >= *deadline {
                // Keys start released. A key that settled back into the state that was
                // already sent did not change, so there is nothing to send.
                let sent = sent_values
                    .get(&event.code)
                    .copied()
                    .unwrap_or(KeyValue::Release);
                if sent != event.value {
                    log::debug!("Emitting deferred event for {:?}", event.code);
                    try_send_panic(process_tx, *event);
                    sent_values.insert(event.code, event.value);
                }
                false // Remove this item from the Vec
            } else {
                true // Keep this item in the Vec
            }
        });

        // Return true if there are still pending events
        !self.pending_events.is_empty()
    }
}
