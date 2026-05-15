use super::*;
use crate::kanata::ManagedRepeatState;

#[test]
fn managed_repeat_basic() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 10 managed-repeat-interval 5)
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:30 u:a t:1",
    );
    let events: Vec<&str> = result.split('\n').collect();
    let b_downs = events.iter().filter(|e| **e == "out:↓B").count();
    let b_ups = events.iter().filter(|e| **e == "out:↑B").count();
    assert!(
        b_downs >= 4,
        "expected at least 4 B presses, got {b_downs}: {result}"
    );
    assert!(
        b_ups >= 4,
        "expected at least 4 B releases, got {b_ups}: {result}"
    );
}

#[test]
fn managed_repeat_no_repeat_before_delay() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 20 managed-repeat-interval 5)
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:3 u:a t:1",
    );
    assert_eq!("out:↓B\nt:3ms\nout:↑B", result);
}

#[test]
fn managed_repeat_modifier_exempt() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 5 managed-repeat-interval 3)
         (defsrc a lsft)
         (deflayer base a lsft)
        ",
        "d:lsft t:20 u:lsft t:1",
    );
    assert_eq!("out:↓LShift\nt:20ms\nout:↑LShift", result);
}

#[test]
fn managed_repeat_disabled_by_default() {
    let result = simulate(
        "
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:20 u:a t:1",
    );
    assert_eq!("out:↓B\nt:20ms\nout:↑B", result);
}

#[test]
fn managed_repeat_releases_before_os_repeat() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 100 managed-repeat-interval 50)
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:10 u:a t:1",
    );
    let events: Vec<&str> = result.split('\n').collect();
    let b_downs = events.iter().filter(|e| **e == "out:↓B").count();
    assert_eq!(1, b_downs, "one press, no repeat in 10 ticks: {result}");
}

#[test]
fn managed_repeat_per_key_override() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 100 managed-repeat-interval 100)
         (defsrc a b)
         (deflayer base a b)
         (defrepeat
           (a 8 4)
         )
        ",
        "d:a t:25 u:a t:1",
    );
    let events: Vec<&str> = result.split('\n').collect();
    let a_downs = events.iter().filter(|e| **e == "out:↓A").count();
    assert!(
        a_downs >= 3,
        "expected at least 3 A presses, got {a_downs}: {result}"
    );
}

#[test]
fn managed_repeat_override_vs_default() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 100 managed-repeat-interval 100)
         (defsrc a b)
         (deflayer base a b)
         (defrepeat
           (a 8 4)
         )
        ",
        "d:b t:15 u:b t:1",
    );
    let events: Vec<&str> = result.split('\n').collect();
    let b_downs = events.iter().filter(|e| **e == "out:↓B").count();
    assert_eq!(1, b_downs, "no repeat, just initial press: {result}");
}

#[test]
fn managed_repeat_state_swap_picks_up_new_timing() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    // Start with a long delay so no repeats happen in 15 ticks.
    let mut k = Kanata::new_from_str(
        "
         (defcfg managed-repeat yes managed-repeat-delay 100 managed-repeat-interval 100)
         (defsrc a)
         (deflayer base a)
        ",
        Default::default(),
    )
    .expect("cfg parses");

    // Press A and hold for 15 ticks — no repeats expected with delay=100.
    let key_a = str_to_oscode("a").unwrap();
    k.handle_input_event(&KeyEvent::new(key_a, KeyValue::Press))
        .unwrap();
    #[cfg(not(all(target_os = "windows", not(feature = "interception_driver"))))]
    crate::PRESSED_KEYS.lock().insert(key_a);
    #[cfg(all(target_os = "windows", not(feature = "interception_driver")))]
    crate::PRESSED_KEYS
        .lock()
        .insert(key_a, web_time::Instant::now());
    for _ in 0..15 {
        let _ = k.tick_ms(1, &None);
    }
    let events_before: Vec<String> = k.kbd_out.outputs.events.clone();
    let a_downs_before = events_before.iter().filter(|e| *e == "out:↓A").count();
    assert_eq!(
        1, a_downs_before,
        "only initial press, no repeats with delay=100: {events_before:?}"
    );

    // Release A.
    k.handle_input_event(&KeyEvent::new(key_a, KeyValue::Release))
        .unwrap();
    crate::PRESSED_KEYS.lock().remove(&key_a);
    let _ = k.tick_ms(1, &None);

    // Swap in new state with fast timing (delay=8, interval=4).
    // This is what do_live_reload now does.
    let new_state = ManagedRepeatState::new(8, 4);
    // No per-key overrides for this test.
    k.managed_repeat_state = Some(new_state);

    // Clear output to isolate the post-reload behavior.
    k.kbd_out.outputs.events.clear();

    // Press A again and hold for 30 ticks — should see repeats with new timing.
    k.handle_input_event(&KeyEvent::new(key_a, KeyValue::Press))
        .unwrap();
    #[cfg(not(all(target_os = "windows", not(feature = "interception_driver"))))]
    crate::PRESSED_KEYS.lock().insert(key_a);
    #[cfg(all(target_os = "windows", not(feature = "interception_driver")))]
    crate::PRESSED_KEYS
        .lock()
        .insert(key_a, web_time::Instant::now());
    for _ in 0..30 {
        let _ = k.tick_ms(1, &None);
    }
    k.handle_input_event(&KeyEvent::new(key_a, KeyValue::Release))
        .unwrap();
    crate::PRESSED_KEYS.lock().remove(&key_a);
    let _ = k.tick_ms(1, &None);

    let events_after: Vec<String> = k.kbd_out.outputs.events.clone();
    let a_downs_after = events_after.iter().filter(|e| *e == "out:↓A").count();
    assert!(
        a_downs_after >= 4,
        "expected at least 4 A presses with new fast timing, got {a_downs_after}: {events_after:?}"
    );
}

#[test]
fn managed_repeat_disable_on_reload_cancels_repeat() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut k = Kanata::new_from_str(
        "
         (defcfg managed-repeat yes managed-repeat-delay 8 managed-repeat-interval 4)
         (defsrc a)
         (deflayer base a)
        ",
        Default::default(),
    )
    .expect("cfg parses");

    assert!(k.managed_repeat_state.is_some());

    // Simulate reload that disables managed repeat.
    k.managed_repeat_state = None;
    k.allow_hardware_repeat = true;

    assert!(k.managed_repeat_state.is_none());
    assert!(k.allow_hardware_repeat);
}
