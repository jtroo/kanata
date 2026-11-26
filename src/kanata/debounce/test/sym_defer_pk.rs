#[cfg(test)]
mod tests {
    use crate::kanata::debounce::debounce::create_debounce_algorithm;
    use crate::kanata::{KeyEvent, OsCode};
    use crate::oskbd::KeyValue;
    use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn basic_functionality() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, 50);

        assert_eq!(algorithm.name(), DebounceAlgorithm::SymDeferPk);
        assert_eq!(algorithm.debounce_time(), 50);

        // Simulate a key press event
        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let has_pending = algorithm.process_event(key_event, &tx);
        assert!(has_pending, "Expected a pending event");

        // Verify no event is sent immediately
        assert!(
            rx.try_recv().is_err(),
            "Expected no event to be sent immediately"
        );

        // Simulate a tick after debounce duration
        thread::sleep(Duration::from_millis(51));
        let has_pending_after_tick = algorithm.tick(&tx, Instant::now());
        assert!(
            !has_pending_after_tick,
            "Expected no pending events after tick"
        );

        // Verify the event was sent
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);
    }

    #[test]
    fn debounce_key_press_and_release() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, 50);

        let key_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);

        // Process key press
        algorithm.process_event(key_press, &tx);
        assert!(
            rx.try_recv().is_err(),
            "Expected no key press event immediately"
        );

        // Process key release
        algorithm.process_event(key_release, &tx);
        assert!(
            rx.try_recv().is_err(),
            "Expected no key release event immediately, this one should be debounced"
        );

        // Simulate a tick after debounce duration
        thread::sleep(Duration::from_millis(51));
        algorithm.tick(&tx, Instant::now());

        // The key settled where it started (released), so nothing is sent
        assert!(rx.try_recv().is_err(), "Expected no events after debounce");
    }

    #[test]
    fn test_repeat_event() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, 50);

        let repeat_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Repeat);

        // Process repeat event
        let has_pending = algorithm.process_event(repeat_event, &tx);
        assert!(
            !has_pending,
            "Repeat events should not create pending deadlines"
        );

        // Verify the repeat event was sent immediately
        let received_event = rx.try_recv().expect("Expected a repeat event");
        assert_eq!(received_event.code, repeat_event.code);
        assert_eq!(received_event.value, repeat_event.value);
    }

    #[test]
    fn test_multiple_keys() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, 50);

        let key_a_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_b_press = KeyEvent::new(OsCode::KEY_B, KeyValue::Press);

        // Process key A press
        algorithm.process_event(key_a_press, &tx);
        assert!(
            rx.try_recv().is_err(),
            "Expected no key A press event immediately"
        );

        // Process key B press
        algorithm.process_event(key_b_press, &tx);
        assert!(
            rx.try_recv().is_err(),
            "Expected no key B press event immediately"
        );

        // Simulate a tick after debounce duration
        thread::sleep(Duration::from_millis(51));
        algorithm.tick(&tx, Instant::now());

        // Verify key A press event
        let press_event_a = rx.try_recv().expect("Expected a key A press event");
        assert_eq!(press_event_a.code, key_a_press.code);
        assert_eq!(press_event_a.value, key_a_press.value);

        // Verify key B press event
        let press_event_b = rx.try_recv().expect("Expected a key B press event");
        assert_eq!(press_event_b.code, key_b_press.code);
        assert_eq!(press_event_b.value, key_b_press.value);
    }

    #[test]
    fn test_reset_deadline_keeps_latest_event() {
        const DEBOUNCE_MS: u64 = 50;
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm =
            create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, DEBOUNCE_MS as u16);

        let key_a_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_a_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);

        // 1. Process initial Press event
        let mut has_pending = algorithm.process_event(key_a_press, &tx);
        assert!(has_pending, "Press event should create a pending deadline");
        assert!(
            rx.try_recv().is_err(),
            "Press event should not be sent immediately"
        );

        // 2. Wait for less than the debounce duration
        thread::sleep(Duration::from_millis(DEBOUNCE_MS / 2)); // e.g., 25ms

        // 3. Process Release event - this should replace the pending Press and reset the deadline
        has_pending = algorithm.process_event(key_a_release, &tx);
        assert!(
            has_pending,
            "Release event should reset the pending deadline"
        );
        assert!(
            rx.try_recv().is_err(),
            "Release event should not be sent immediately, nor should the original press"
        );

        // 4. Wait until after the *new* deadline has passed
        // Original deadline was start + 50ms.
        // New deadline is (start + 25ms) + 50ms = start + 75ms.
        // We need to sleep until slightly after start + 75ms.
        // We already slept 25ms, so sleep for another 51ms+.
        thread::sleep(Duration::from_millis(DEBOUNCE_MS + 1)); // e.g., 51ms (total sleep ~76ms)

        // 5. Tick the algorithm
        let has_pending_after_tick = algorithm.tick(&tx, Instant::now());
        assert!(
            !has_pending_after_tick,
            "Expected no pending events after the reset deadline passed"
        );

        // 6. The latest state is Release, which is where the key started, so nothing is sent
        assert!(rx.try_recv().is_err(), "Expected no events after the tick");
    }

    fn drain(rx: &mpsc::Receiver<KeyEvent>) -> Vec<(OsCode, KeyValue)> {
        rx.try_iter().map(|ev| (ev.code, ev.value)).collect()
    }

    #[test]
    fn latest_state_is_sent() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, 20);

        // Press chatter that settles pressed.
        for value in [KeyValue::Press, KeyValue::Release, KeyValue::Press] {
            algorithm.process_event(KeyEvent::new(OsCode::KEY_A, value), &tx);
        }
        thread::sleep(Duration::from_millis(30));
        algorithm.tick(&tx, Instant::now());
        assert_eq!(drain(&rx), vec![(OsCode::KEY_A, KeyValue::Press)]);

        // Release chatter that settles released.
        for value in [KeyValue::Release, KeyValue::Press, KeyValue::Release] {
            algorithm.process_event(KeyEvent::new(OsCode::KEY_A, value), &tx);
        }
        thread::sleep(Duration::from_millis(30));
        algorithm.tick(&tx, Instant::now());
        assert_eq!(drain(&rx), vec![(OsCode::KEY_A, KeyValue::Release)]);
    }

    #[test]
    fn bounce_back_to_sent_state_sends_nothing() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymDeferPk, 20);

        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Press), &tx);
        thread::sleep(Duration::from_millis(30));
        algorithm.tick(&tx, Instant::now());
        assert_eq!(drain(&rx), vec![(OsCode::KEY_A, KeyValue::Press)]);

        // Held, and the switch bounces open and closed again.
        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Release), &tx);
        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Press), &tx);
        thread::sleep(Duration::from_millis(30));
        algorithm.tick(&tx, Instant::now());
        assert_eq!(drain(&rx), vec![], "the key never stopped being held");
    }
}
