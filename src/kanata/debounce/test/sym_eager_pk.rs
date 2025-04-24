#[cfg(test)]
mod tests {
    use crate::kanata::{KeyEvent, OsCode};
    use crate::oskbd::KeyValue;
    use crate::kanata::debounce::debounce::create_debounce_algorithm;
    use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;
    use std::sync::mpsc;

    #[test]
    fn basic_functionality() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        assert_eq!(algorithm.name(), DebounceAlgorithm::SymEagerPk);
        assert_eq!(algorithm.debounce_time(), 50);

        // Simulate a key press event
        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let has_pending = algorithm.process_event(key_event, &tx);
        assert!(!has_pending);

        // Verify the press event was immediately sent
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);

        // Simulate a key release event
        let key_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);
        let has_pending = algorithm.process_event(key_release, &tx);
        assert!(!has_pending);

        // Verify the release event was debounced
        assert!(rx.try_recv().is_err(), "Expected no event to be sent");

        // Wait for 51ms to allow more events to be processed
        std::thread::sleep(std::time::Duration::from_millis(51));

        // Simulate a press event again
        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let has_pending = algorithm.process_event(key_event, &tx);
        assert!(!has_pending);
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);

        // Wait for 51ms to allow the next event to be processed
        std::thread::sleep(std::time::Duration::from_millis(51));
        // Send release event
        let key_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);
        let has_pending = algorithm.process_event(key_release, &tx);
        assert!(!has_pending);
        let release_event = rx.try_recv().expect("Expected a release event");
        assert_eq!(release_event.code, key_release.code);
        assert_eq!(release_event.value, key_release.value);

    }

    #[test]
    fn debounce_key_press() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);

        // First key press should be processed
        let has_pending = algorithm.process_event(key_event, &tx);
        assert!(!has_pending);
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code);
        assert_eq!(received_event.value, key_event.value);

        // Second key press within debounce duration should be ignored
        std::thread::sleep(std::time::Duration::from_millis(30));
        let has_pending = algorithm.process_event(key_event, &tx);
        assert!(!has_pending);
        assert!(rx.try_recv().is_err(), "Expected no event to be sent");
    }

    #[test]
    fn test_repeat_event() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        let repeat_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Repeat);

        // Process repeat event
        let has_pending = algorithm.process_event(repeat_event, &tx);
        assert!(!has_pending, "Repeat events should not create pending deadlines");

        // Verify the repeat event was sent immediately
        let received_event = rx.try_recv().expect("Expected a repeat event");
        assert_eq!(received_event.code, repeat_event.code);
        assert_eq!(received_event.value, repeat_event.value);
    }

    #[test]
    fn test_multiple_keys() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::SymEagerPk, 50);

        let key_a_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_b_press = KeyEvent::new(OsCode::KEY_B, KeyValue::Press);

        // Process key A press
        algorithm.process_event(key_a_press, &tx);
        let received_event = rx.try_recv().expect("Expected a key A press event");
        assert_eq!(received_event.code, key_a_press.code);
        assert_eq!(received_event.value, key_a_press.value);

        // Process key B press
        algorithm.process_event(key_b_press, &tx);
        let received_event = rx.try_recv().expect("Expected a key B press event");
        assert_eq!(received_event.code, key_b_press.code);
        assert_eq!(received_event.value, key_b_press.value);

        // Simulate debounce duration for key A
        std::thread::sleep(std::time::Duration::from_millis(51));
        let key_a_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);
        algorithm.process_event(key_a_release, &tx);
        let release_event = rx.try_recv().expect("Expected a key A release event");
        assert_eq!(release_event.code, key_a_release.code);
        assert_eq!(release_event.value, key_a_release.value);
    }
}