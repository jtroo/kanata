use crate::kanata::debounce::asym_eager_defer_pk::AsymEagerDeferPk;
use crate::{kanata::KeyEvent, sym_defer_pk::SymDeferPk, sym_eager_pk::SymEagerPk};
use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
use std::{sync::mpsc::SyncSender as Sender, time::Instant};

/// Trait for debounce algorithms
pub trait Debounce: Send + Sync {
    /// Returns the name of the debounce algorithm
    fn name(&self) -> DebounceAlgorithm;

    /// Returns the debounce time in milliseconds
    fn debounce_time(&self) -> u16;

    /// Handle one key event that happened at `now`, sending whatever it lets through.
    /// Returns whether there are pending events, which need `tick` to be called.
    fn process_event(
        &mut self,
        event: KeyEvent,
        process_tx: &Sender<KeyEvent>,
        now: Instant,
    ) -> bool;

    /// Optional tick function to process delayed events (deadlines),
    /// returns whether there are pending events
    fn tick(&mut self, _process_tx: &Sender<KeyEvent>, _now: Instant) -> bool {
        false // Default implementation: no pending events
    }
}

/// Factory function to create debounce algorithm instances
pub fn create_debounce_algorithm(
    algorithm: DebounceAlgorithm,
    debounce_duration_ms: u16,
) -> Box<dyn Debounce> {
    match algorithm {
        DebounceAlgorithm::AsymEagerDeferPk => {
            Box::new(AsymEagerDeferPk::new(debounce_duration_ms))
        }
        DebounceAlgorithm::SymEagerPk => Box::new(SymEagerPk::new(debounce_duration_ms)),
        DebounceAlgorithm::SymDeferPk => Box::new(SymDeferPk::new(debounce_duration_ms)),
    }
}

/// Helper function to send events and panic on failure
pub fn try_send_panic(tx: &Sender<KeyEvent>, kev: KeyEvent) {
    if let Err(e) = tx.try_send(kev) {
        panic!("failed to send on channel: {e:?}");
    }
}
