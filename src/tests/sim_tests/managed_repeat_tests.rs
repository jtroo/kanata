use super::*;

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
