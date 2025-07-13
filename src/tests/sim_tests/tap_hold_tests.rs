use super::*;

#[test]
fn delayed_timedout_released_taphold_can_still_tap() {
    let result = simulate(
        "
        (defcfg concurrent-tap-hold yes )
        (defsrc a j )
        (deflayer base @a @j)
        (defalias
         a (tap-hold 200 1000 a lctl)
         j (tap-hold 200 500 j lsft))
        ",
        "d:a t:100 d:j t:10 u:j t:1100 u:a t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:999ms dn:LCtrl t:2ms dn:J t:6ms up:J t:203ms up:LCtrl",
        result
    );
}

#[test]
fn tap_hold_release_timeout_no_reset() {
    let result = simulate(
        "
        (defsrc a)
        (deflayer l1 (tap-hold-release-timeout 100 100 x y z))
        ",
        "d:a t:50 d:b t:75 u:b u:a t:25",
    )
    .to_ascii();
    assert_eq!("t:100ms dn:Z t:1ms dn:B t:24ms up:B t:1ms up:Z", result);
}

#[test]
fn tap_hold_release_timeout_with_reset() {
    let result = simulate(
        "
        (defsrc a)
        (deflayer l1 (tap-hold-release-timeout 100 100 x y z reset-timeout-on-press))
        ",
        "d:a t:50 d:b t:75 u:b u:a t:25",
    )
    .to_ascii();
    assert_eq!("t:125ms dn:Y t:6ms dn:B t:1ms up:B t:1ms up:Y", result);
}
