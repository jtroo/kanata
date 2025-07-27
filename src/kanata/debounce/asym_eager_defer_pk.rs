use std::collections::HashMap;
use std::time::{Duration, Instant};
use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;

use crate::kanata::{KeyEvent, KeyValue, OsCode};
use std::sync::mpsc::SyncSender as Sender;
use crate::kanata::debounce::debounce::{try_send_panic, Debounce};

/// Implementation of the asym_eager_defer_pk algorithm
/// See: https://github.com/qmk/qmk_firmware/blob/6ef97172889ccd5db376b2a9f8825489e24fdac4/docs/feature_debounce_type.md
pub struct AsymEagerDeferPk {
    debounce_duration: Duration,
    last_key_event_time: HashMap<OsCode, Instant>,
    release_deadlines: Vec<(OsCode, Instant)>,
}

impl AsymEagerDeferPk {
    pub fn new(debounce_duration_ms: u16) -> Self {
        Self {
            debounce_duration: Duration::from_millis(debounce_duration_ms.into()),
            last_key_event_time: HashMap::new(),
            release_deadlines: Vec::new(), // Initialize as an empty Vec
        }
    }
}

impl Debounce for AsymEagerDeferPk {
    fn name(&self) -> DebounceAlgorithm {
        DebounceAlgorithm::AsymEagerDeferPk
    }

    fn debounce_time(&self) -> u16 {
        self.debounce_duration.as_millis() as u16
    }

    fn process_event(&mut self, event: KeyEvent, process_tx: &Sender<KeyEvent>) -> bool {
        let now = Instant::now();
        let oscode = event.code;

        match event.value {
            KeyValue::Press => {
                // Cancel any pending release for this key
                self.release_deadlines.retain(|(code, _)| *code != oscode);

                // Check if the key press is within the debounce duration
                if let Some(&last_time) = self.last_key_event_time.get(&oscode) {
                    if now.duration_since(last_time) < self.debounce_duration {
                        log::debug!("Debouncing key press for {:?}", oscode);
                        return !self.release_deadlines.is_empty(); // Skip processing this event
                    }
                }

                // Eagerly process key-down events
                self.last_key_event_time.insert(oscode, now);
                try_send_panic(process_tx, event);
            }
            KeyValue::Release => {
                // Check if pending release event is already scheduled
                if self.release_deadlines.iter().any(|(code, _)| *code == oscode) {
                    log::debug!("Release event already scheduled for {:?}", oscode);
                    return !self.release_deadlines.is_empty(); // Skip processing this event
                }
                // Schedule the release event for later
                self.release_deadlines.push((oscode, now + self.debounce_duration));
            }
            KeyValue::Repeat => {
                // Forward repeat events immediately
                log::debug!("Forwarding repeat event for {:?}", oscode);
                try_send_panic(process_tx, event);
            }
            _ => {
                // Forward other key events without debouncing
                log::debug!("Forwarding other event for {:?}", oscode);
                try_send_panic(process_tx, event);
            }
        }

        // Return true if there are still pending deadlines
        !self.release_deadlines.is_empty()
    }

    fn tick(&mut self, process_tx: &Sender<KeyEvent>, now: Instant) -> bool {
        // Process any release events whose deadlines have passed
        self.release_deadlines.retain(|(oscode, deadline)| {
            if now >= *deadline {
                log::debug!("Emitting key release for {:?}", oscode);
                let release_event = KeyEvent {
                    code: *oscode,
                    value: KeyValue::Release,
                };
                try_send_panic(process_tx, release_event);
                false // Remove this item from the Vec
            } else {
                true // Keep this item in the Vec
            }
        });

        // Return true if there are still pending deadlines
        !self.release_deadlines.is_empty()
    }
}
