use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::kanata::{KeyEvent, KeyValue, OsCode};
use std::sync::mpsc::SyncSender as Sender;
use crate::kanata::debounce::debounce::{try_send_panic, Debounce};

/// Implementation of the asym_eager_defer_pk algorithm
/// See: https://github.com/qmk/qmk_firmware/blob/6ef97172889ccd5db376b2a9f8825489e24fdac4/docs/feature_debounce_type.md
pub struct AsymEagerDeferPk {
    debounce_duration: Duration,
    last_key_event_time: HashMap<OsCode, Instant>,
    release_deadlines: HashMap<OsCode, Instant>,
}

impl AsymEagerDeferPk {
    pub fn new(debounce_duration_ms: u16) -> Self {
        Self {
            debounce_duration: Duration::from_millis(debounce_duration_ms.into()),
            last_key_event_time: HashMap::new(),
            release_deadlines: HashMap::new(),
        }
    }

    pub fn tick(&mut self, process_tx: &Sender<KeyEvent>, now: Instant) {
        // Process any release events whose deadlines have passed
        let mut to_remove = vec![];
        for (&oscode, &deadline) in &self.release_deadlines {
            if now >= deadline {
                log::info!("Emitting key release for {:?}", oscode);
                let release_event = KeyEvent {
                    code: oscode,
                    value: KeyValue::Release,
                };
                try_send_panic(process_tx, release_event);
                to_remove.push(oscode);
            }
        }
        for oscode in to_remove {
            self.release_deadlines.remove(&oscode);
        }
    }
}

impl Debounce for AsymEagerDeferPk {
    fn process_event(&mut self, event: KeyEvent, process_tx: &Sender<KeyEvent>) {
        let now = Instant::now();
        let oscode = event.code;

        match event.value {
            KeyValue::Press => {
                // Cancel any pending release for this key
                self.release_deadlines.remove(&oscode);

                // Check if the key press is within the debounce duration
                if let Some(&last_time) = self.last_key_event_time.get(&oscode) {
                    if now.duration_since(last_time) < self.debounce_duration {
                        log::info!("Debouncing key press for {:?}", oscode);
                        return; // Skip processing this event
                    }
                }

                // Eagerly process key-down events
                self.last_key_event_time.insert(oscode, now);
                try_send_panic(process_tx, event);
            }
            KeyValue::Release => {
                // Schedule the release event for later
                self.release_deadlines.insert(oscode, now + self.debounce_duration);
            }
            KeyValue::Repeat => {
                // Forward repeat events immediately
                log::info!("Forwarding repeat event for {:?}", oscode);
                try_send_panic(process_tx, event);
            }
            _ => {
                // Forward other key events without debouncing
                log::debug!("Forwarding other event for {:?}", oscode);
                try_send_panic(process_tx, event);
            }
        }
    }
}
