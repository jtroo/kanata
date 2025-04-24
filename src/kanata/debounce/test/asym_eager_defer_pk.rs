#[cfg(test)]
mod tests {
    use kanata_parser::{cfg::debounce_algorithm::DebounceAlgorithm, keys::OsCode};
    use crate::{debounce::debounce::create_debounce_algorithm, kanata::KeyEvent, oskbd::KeyValue};
    use std::{sync::mpsc, time::Instant};

    #[test]
    fn basic_functionality() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::AsymEagerDeferPk, 50);

        assert_eq!(algorithm.name(), DebounceAlgorithm::AsymEagerDeferPk);
        assert_eq!(algorithm.debounce_time(), 50);

        // Simulate a key press event
        let key_event = KeyEvent::new(OsCode::KEY_A, KeyValue::Press); // Key code A, pressed
        let has_pending = algorithm.process_event(key_event, &tx);
        assert!(!has_pending);

        // Verify the press event was immediately sent
        let received_event = rx.try_recv().expect("Expected an event");
        assert_eq!(received_event.code, key_event.code, "Key codes do not match");
        assert_eq!(received_event.value, key_event.value, "Key values do not match");

        let key_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release); // Key code A, released
        let has_pending = algorithm.process_event(key_release, &tx);
        assert!(has_pending, "Expected a pending release event");

        // Simulate a tick
        let now = Instant::now();
        let has_pending_after_tick = algorithm.tick(&tx, now);
        assert!(has_pending_after_tick, "Expected release event to be pending");

        // Wait for 51ms to ensure the release event is processed
        std::thread::sleep(std::time::Duration::from_millis(51));
        let has_pending_after_tick = algorithm.tick(&tx, Instant::now());
        assert!(!has_pending_after_tick, "Expected release event to be processed");

        // Verify the release event was sent
        let release_event = rx.try_recv().expect("Expected a release event");
        assert_eq!(release_event.code, key_release.code, "Key codes do not match");
        assert_eq!(release_event.value, key_release.value, "Key values do not match");
        assert!(rx.try_recv().is_err(), "Expected no more events in the channel");
    }

    #[test]
    fn debounce_key_press() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::AsymEagerDeferPk, 50);

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
    fn debounce_key_release() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::AsymEagerDeferPk, 50);

        let key_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);

        // Process key press
        algorithm.process_event(key_press, &tx);
        let received_event = rx.try_recv().expect("Expected a key press event");
        assert_eq!(received_event.code, key_press.code);
        assert_eq!(received_event.value, key_press.value);

        // Process key release
        let has_pending = algorithm.process_event(key_release, &tx);
        assert!(has_pending, "Expected a pending release event");

        // Verify no release event is sent immediately
        assert!(rx.try_recv().is_err(), "Expected no release event yet");

        // Another press event within debounce duration should remove the pending release
        // and not send a new event
        std::thread::sleep(std::time::Duration::from_millis(30));
        let has_pending = algorithm.process_event(key_press, &tx);
        assert!(!has_pending, "No pending event expected");
        assert!(rx.try_recv().is_err(), "Expected no event to be sent, because of debounce");

        // Because of the new key release, the timer should be reset
        let has_pending = algorithm.process_event(key_release, &tx);
        assert!(has_pending, "Expected a pending release event");
        std::thread::sleep(std::time::Duration::from_millis(30));
        // note that 30ms + 30ms = 60ms > 50ms
        // Simulate a tick before the debounce duration
        let now = Instant::now();
        let has_pending_after_tick = algorithm.tick(&tx, now);
        assert!(has_pending_after_tick, "Expected release event to be pending");

        // Simulate a tick after debounce duration
        std::thread::sleep(std::time::Duration::from_millis(21));
        let has_pending_after_tick = algorithm.tick(&tx, Instant::now());
        assert!(!has_pending_after_tick, "Expected no pending events after tick");

        // Verify the release event was sent
        let release_event = rx.try_recv().expect("Expected a release event");
        assert_eq!(release_event.code, key_release.code);
        assert_eq!(release_event.value, key_release.value);
    }

    #[test]
    fn repeat_event() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::AsymEagerDeferPk, 50);

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
    fn keys_are_handled_independently() {
        let (tx, rx) = mpsc::sync_channel(10);
        let mut algorithm = create_debounce_algorithm(DebounceAlgorithm::AsymEagerDeferPk, 50);

        let key_a_press = KeyEvent::new(OsCode::KEY_A, KeyValue::Press);
        let key_b_press = KeyEvent::new(OsCode::KEY_B, KeyValue::Press);
        let key_a_release = KeyEvent::new(OsCode::KEY_A, KeyValue::Release);
        let key_b_release = KeyEvent::new(OsCode::KEY_B, KeyValue::Release);

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

        // Process key A release
        algorithm.process_event(key_a_release, &tx);
        assert!(rx.try_recv().is_err(), "Expected no key A release event yet");

        // Process key B release
        algorithm.process_event(key_b_release, &tx);
        assert!(rx.try_recv().is_err(), "Expected no key B release event yet");

        // Simulate a tick after debounce duration
        std::thread::sleep(std::time::Duration::from_millis(51));
        algorithm.tick(&tx, Instant::now());

        // Verify key A release event
        let release_event = rx.try_recv().expect("Expected a key A release event");
        assert_eq!(release_event.code, key_a_release.code);
        assert_eq!(release_event.value, key_a_release.value);

        // Verify key B release event
        let release_event = rx.try_recv().expect("Expected a key B release event");
        assert_eq!(release_event.code, key_b_release.code);
        assert_eq!(release_event.value, key_b_release.value);
    }
}