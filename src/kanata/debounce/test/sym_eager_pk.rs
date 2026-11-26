#[cfg(test)]
mod tests {
    use crate::kanata::debounce::debounce::create_debounce_algorithm;
    use crate::kanata::{KeyEvent, OsCode};
    use crate::oskbd::KeyValue;
    use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn basic_functionality() {
        let mut now = Instant::now();
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        assert_eq!(algorithm.name(), DebounceAlgorithm::SymEagerPk);
        assert_eq!(algorithm.debounce_time(), 50);

        // Simulate a key press event
        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let has_pending = algorithm.process_event(key_event, &tx, now);
        assert!(has_pending, "Expected the key to be locked out");

        // Verify the press event was immediately sent
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);

        // Simulate a key release event
        let key_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);
        let has_pending = algorithm.process_event(key_release, &tx, now);
        assert!(has_pending);

        // Verify the release event was held back during the lockout
        assert!(rx.try_recv().is_err(), "Expected no event to be sent");

        // Once the lockout ends, the release is sent and the key is locked out again
        now += Duration::from_millis(51);
        let has_pending = algorithm.tick(&tx, now);
        assert!(has_pending);
        let release_event = rx.try_recv().expect("Expected a release event");
        assert_eq!(release_event.code, key_release.code);
        assert_eq!(release_event.value, key_release.value);

        // Nothing changed during the second lockout
        now += Duration::from_millis(51);
        let has_pending = algorithm.tick(&tx, now);
        assert!(!has_pending);
        assert!(rx.try_recv().is_err(), "Expected no event to be sent");

        // Simulate a press event again
        let has_pending = algorithm.process_event(key_event, &tx, now);
        assert!(has_pending);
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);
    }

    #[test]
    fn debounce_key_press() {
        let mut now = Instant::now();
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);

        // First key press should be processed
        let has_pending = algorithm.process_event(key_event, &tx, now);
        assert!(has_pending);
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);

        // Second key press within debounce duration should be ignored
        now += Duration::from_millis(30);
        let has_pending = algorithm.process_event(key_event, &tx, now);
        assert!(has_pending);
        assert!(rx.try_recv().is_err(), "Expected no event to be sent");
    }

    #[test]
    fn test_repeat_event() {
        let now = Instant::now();
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        let repeat_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Repeat);

        // Process repeat event
        let has_pending = algorithm.process_event(repeat_event, &tx, now);
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
        let mut now = Instant::now();
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        let key_a_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_b_press = KeyEvent::new(OsCode::KEY_B, KeyValue::Press);

        // Process key A press
        algorithm.process_event(key_a_press, &tx, now);
        let received_event = rx.try_recv().expect("Expected a key A press event");
        assert_eq!(received_event.code, key_a_press.code);
        assert_eq!(received_event.value, key_a_press.value);

        // Process key B press
        algorithm.process_event(key_b_press, &tx, now);
        let received_event = rx.try_recv().expect("Expected a key B press event");
        assert_eq!(received_event.code, key_b_press.code);
        assert_eq!(received_event.value, key_b_press.value);

        // Simulate debounce duration for key A
        now += Duration::from_millis(51);
        let key_a_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);
        algorithm.process_event(key_a_release, &tx, now);
        let release_event = rx.try_recv().expect("Expected a key A release event");
        assert_eq!(release_event.code, key_a_release.code);
        assert_eq!(release_event.value, key_a_release.value);
    }

    fn drain(rx: &mpsc::Receiver<KeyEvent>) -> Vec<(OsCode, KeyValue)> {
        rx.try_iter().map(|ev| (ev.code, ev.value)).collect()
    }

    #[test]
    fn change_during_lockout_is_sent_when_it_ends() {
        let mut now = Instant::now();
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 20);

        // A tap shorter than the debounce time.
        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Press), &tx, now);
        let has_pending =
            algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Release), &tx, now);
        assert!(
            has_pending,
            "the release has to be sent once the lockout ends"
        );
        assert_eq!(drain(&rx), vec![(OsCode::KEY_A, KeyValue::Press)]);

        now += Duration::from_millis(30);
        algorithm.tick(&tx, now);
        assert_eq!(drain(&rx), vec![(OsCode::KEY_A, KeyValue::Release)]);
    }

    #[test]
    fn bounce_back_to_sent_state_sends_nothing() {
        let mut now = Instant::now();
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 20);

        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Press), &tx, now);
        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Release), &tx, now);
        algorithm.process_event(KeyEvent::new(OsCode::KEY_A, KeyValue::Press), &tx, now);
        now += Duration::from_millis(30);
        let has_pending = algorithm.tick(&tx, now);
        assert!(!has_pending);
        assert_eq!(drain(&rx), vec![(OsCode::KEY_A, KeyValue::Press)]);
    }
}
