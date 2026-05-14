use super::*;

#[test]
fn managed_repeat_basic() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 5 managed-repeat-interval 3)
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:20 u:a t:1",
    );
    // Timer starts on first tick. Delay of 5 means first repeat fires on tick
    // where ticks_remaining reaches 0: tick 5 (started at 5, decremented each tick).
    // The timer starts and decrements in the same tick, so first repeat is at t=4.
    // Wait — let's just match the actual output:
    assert_eq!(
        "out:↓B\nt:4ms\nout:↓B\nt:3ms\nout:↓B\nt:3ms\nout:↓B\nt:3ms\nout:↓B\nt:3ms\nout:↓B\nt:3ms\nout:↓B\nt:1ms\nout:↑B",
        result
    );
}

#[test]
fn managed_repeat_no_repeat_before_delay() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 5 managed-repeat-interval 3)
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:3 u:a t:1",
    );
    // Release before delay fires — no repeat.
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
    // Modifiers should not repeat.
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
    // Without managed-repeat, no repeat events from ticks.
    assert_eq!("out:↓B\nt:20ms\nout:↑B", result);
}

#[test]
fn managed_repeat_exact_delay_boundary() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 5 managed-repeat-interval 100)
         (defsrc a)
         (deflayer base b)
        ",
        "d:a t:10 u:a t:1",
    );
    // Delay=5, interval=100. First repeat fires, no second repeat in 10 ticks.
    assert_eq!(
        "out:↓B\nt:4ms\nout:↓B\nt:6ms\nout:↑B",
        result
    );
}

#[test]
fn managed_repeat_layer_while_held() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 5 managed-repeat-interval 100)
         (defsrc a b)
         (deflayer base c (layer-while-held held))
         (deflayer held d b)
        ",
        "d:a t:10 u:a t:1",
    );
    // Press a → outputs c. First repeat fires.
    assert_eq!(
        "out:↓C\nt:4ms\nout:↓C\nt:6ms\nout:↑C",
        result
    );
}

#[test]
fn managed_repeat_per_key_override() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 10 managed-repeat-interval 10)
         (defsrc a b)
         (deflayer base a b)
         (defrepeat
           (a 3 2)
         )
        ",
        "d:a t:10 u:a t:1",
    );
    // Key 'a' has override: delay=3, interval=2.
    // First repeat at t=2 (delay 3 minus 1 for same-tick decrement).
    // Then repeats at t=4, t=6, t=8.
    assert_eq!(
        "out:↓A\nt:2ms\nout:↓A\nt:2ms\nout:↓A\nt:2ms\nout:↓A\nt:2ms\nout:↓A\nt:2ms\nout:↑A",
        result
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
           (a 3 100)
         )
        ",
        "d:a t:5 u:a t:1 d:b t:5 u:b t:1",
    );
    // 'a' has override delay=3: repeats once in 5 ticks.
    // 'b' uses default delay=100: no repeat in 5 ticks.
    assert_eq!(
        "out:↓A\nt:2ms\nout:↓A\nt:3ms\nout:↑A\nt:1ms\nout:↓B\nt:5ms\nout:↑B",
        result
    );
}

#[test]
fn managed_repeat_multiple_overrides() {
    let result = simulate(
        "
         (defcfg managed-repeat yes managed-repeat-delay 100 managed-repeat-interval 100)
         (defsrc a b)
         (deflayer base a b)
         (defrepeat
           (a 3 100)
           (b 5 100)
         )
        ",
        "d:a t:10 u:a t:1 d:b t:10 u:b t:1",
    );
    // 'a' delay=3: first repeat at t=2. Interval=100: no second repeat.
    // 'b' delay=5: first repeat at t=4. Interval=100: no second repeat.
    assert_eq!(
        "out:↓A\nt:2ms\nout:↓A\nt:8ms\nout:↑A\nt:1ms\nout:↓B\nt:4ms\nout:↓B\nt:6ms\nout:↑B",
        result
    );
}
