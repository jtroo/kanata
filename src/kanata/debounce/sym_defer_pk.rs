use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::kanata::{KeyEvent, KeyValue, OsCode};
use std::sync::mpsc::SyncSender as Sender;
use crate::kanata::debounce::debounce::{try_send_panic, Debounce};

/// Implementation of the sym_defer_pk algorithm
/// Debouncing per key. On any state change, a per-key timer is set.
/// When DEBOUNCE milliseconds of no changes have occurred on that key,
/// the key status change is pushed.
pub struct SymDeferPk {
    debounce_duration: Duration,
    pending_events: HashMap<OsCode, (KeyEvent, Instant)>,
}

impl SymDeferPk {
    pub fn new(debounce_duration_ms: u16) -> Self {
        Self {
            debounce_duration: Duration::from_millis(debounce_duration_ms.into()),
            pending_events: HashMap::new(),
        }
    }
}

impl Debounce for SymDeferPk {
    fn name(&self) -> &str {
        "sym_defer_pk"
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
                // Defer all other events

                // Check if there is a pending event for this key
                if let Some(&(ref _pending_event, _deadline)) = self.pending_events.get(&oscode) {
                    // Skip this events since it is within the debounce duration, pending release
                } else {
                    log::debug!(
                    "Deferring event for {:?} (value: {:?}) until debounce duration passes",
                    oscode,
                        event.value
                    );
                    self.pending_events.insert(oscode, (event, now + self.debounce_duration));
                }
            }
        }

        // Return true if there are still pending events
        !self.pending_events.is_empty()
    }

    fn tick(&mut self, process_tx: &Sender<KeyEvent>, now: Instant) -> bool {
        // Process any events whose debounce duration has passed
        let mut to_remove = vec![];
        for (&oscode, &(ref event, deadline)) in &self.pending_events {
            if now >= deadline {
                log::debug!("Emitting deferred event for {:?}", oscode);
                try_send_panic(process_tx, event.clone());
                to_remove.push(oscode);
            }
        }
        for oscode in to_remove {
            self.pending_events.remove(&oscode);
        }

        // Return true if there are still pending events
        !self.pending_events.is_empty()
    }
}
