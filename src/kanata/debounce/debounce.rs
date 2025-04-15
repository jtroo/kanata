use crate::kanata::KeyEvent;
use std::{sync::mpsc::SyncSender as Sender, time::Instant};
use crate::kanata::debounce::asym_eager_defer_pk::AsymEagerDeferPk;

/// Trait for debounce algorithms
pub trait Debounce: Send + Sync {
    /// Returns the name of the debounce algorithm
    fn name(&self) -> &str;

    /// Returns the debounce time in milliseconds
    fn debounce_time(&self) -> u16;
    
    fn process_event(&mut self, event: KeyEvent, process_tx: &Sender<KeyEvent>) -> bool;

    /// Optional tick function to process delayed events (deadlines),
    /// returns whether there are pending events
    fn tick(&mut self, _process_tx: &Sender<KeyEvent>, _now: Instant) -> bool {
        return false; // Default implementation: no pending events
    }
}

pub fn create_debounce_algorithm(algorithm: &str, debounce_duration_ms: u16) -> Box<dyn Debounce> {
    log::info!("Creating debounce algorithm: {}, duration: {} ms", algorithm, debounce_duration_ms);
    match algorithm {
        "asym_eager_defer_pk" => Box::new(AsymEagerDeferPk::new(debounce_duration_ms)),
        // Add other algorithms here
        _ => panic!("Unknown debounce algorithm: {}", algorithm),
    }
}

/// Helper function to send events and panic on failure
pub fn try_send_panic(tx: &Sender<KeyEvent>, kev: KeyEvent) {
    if let Err(e) = tx.try_send(kev) {
        panic!("failed to send on channel: {e:?}");
    }
}
