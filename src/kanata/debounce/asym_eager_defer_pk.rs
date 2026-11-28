use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::kanata::debounce::debounce::{Debounce, try_send_panic};
use crate::kanata::{KeyEvent, KeyValue, OsCode};
use std::sync::mpsc::SyncSender as Sender;

/// Implementation of the asym_eager_defer_pk algorithm
/// See: https://github.com/qmk/qmk_firmware/blob/6ef97172889ccd5db376b2a9f8825489e24fdac4/docs/feature_debounce_type.md
pub struct AsymEagerDeferPk {
    debounce_duration: Duration,
    last_key_event_time: HashMap<OsCode, Instant>,
    release_deadlines: Vec<(KeyEvent, Instant)>,
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

    fn process_event(
        &mut self,
        event: KeyEvent,
        process_tx: &Sender<KeyEvent>,
        now: Instant,
    ) -> bool {
        let oscode = event.code;

        match event.value {
            KeyValue::Press => {
                // A press while a release is pending means the release was a bounce: the key
                // never left the pressed state that was already sent, so send nothing.
                let pending_releases = self.release_deadlines.len();
                self.release_deadlines.retain(|(ev, _)| ev.code != oscode);
                if self.release_deadlines.len() != pending_releases {
                    log::debug!("Debouncing key release for {:?}", oscode);
                    return !self.release_deadlines.is_empty();
                }

                // Check if the key press is within the debounce duration
                if let Some(&last_time) = self.last_key_event_time.get(&oscode)
                    && now.duration_since(last_time) < self.debounce_duration
                {
                    log::debug!("Debouncing key press for {:?}", oscode);
                    return !self.release_deadlines.is_empty(); // Skip processing this event
                }

                // Eagerly process key-down events
                self.last_key_event_time.insert(oscode, now);
                try_send_panic(process_tx, event);
            }
            KeyValue::Release => {
                // Check if pending release event is already scheduled
                if self
                    .release_deadlines
                    .iter()
                    .any(|(ev, _)| ev.code == oscode)
                {
                    log::debug!("Release event already scheduled for {:?}", oscode);
                    return !self.release_deadlines.is_empty(); // Skip processing this event
                }
                // Schedule the release event for later
                self.release_deadlines
                    .push((event, now + self.debounce_duration));
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
        self.release_deadlines.retain(|(release_event, deadline)| {
            if now >= *deadline {
                log::debug!("Emitting key release for {:?}", release_event.code);
                try_send_panic(process_tx, *release_event);
                false // Remove this item from the Vec
            } else {
                true // Keep this item in the Vec
            }
        });

        // Return true if there are still pending deadlines
        !self.release_deadlines.is_empty()
    }
}
