use crate::kanata::KeyEvent;
use std::sync::mpsc::SyncSender as Sender;
use crate::kanata::debounce::asym_eager_defer_pk::AsymEagerDeferPk;

/// Trait for debounce algorithms
pub trait Debounce {
    fn process_event(&mut self, event: KeyEvent, process_tx: &Sender<KeyEvent>);
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
