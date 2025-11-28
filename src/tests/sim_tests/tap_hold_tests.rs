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

#[test]
fn on_physical_idle_with_tap_repress() {
    let result = simulate(
        "
(defsrc a)
(deflayer base @a)
(deflayer nomods a)
(defvirtualkeys to-base (layer-switch base))
(defalias
  tap (multi
    (layer-switch nomods)
    (on-physical-idle 20 tap-vkey to-base)
  )
  a (tap-hold 100 100 (multi a @tap) b)
)
        ",
        "d:a t:30 u:a t:30 d:a t:1000",
    )
    .to_ascii();
    // t:30ms dn:A t:6ms up:A t:124ms dn:B
    assert_eq!("t:30ms dn:A t:6ms up:A t:24ms dn:A", result);
}

#[test]
fn tap_hold_release_tap_keys_release() {
    let cfg = "
        (defsrc a b c)
        (deflayer l1
         (tap-hold-release-tap-keys-release 100 100 x y (v) (z))
         (tap-hold-release-tap-keys-release 100 100 x y (w) (z v))
         (tap-hold-release-tap-keys-release 100 100 x y () (z v))
        )
    ";
    let result = simulate(cfg, "d:a t:20 u:a t:20 d:a t:200").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms up:X t:14ms dn:X", result);
    let result = simulate(cfg, "d:a t:20 u:a t:200 d:a t:200").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms up:X t:294ms dn:Y", result);
    let result = simulate(cfg, "d:a t:50 u:a t:50").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms up:X", result);
    let result = simulate(cfg, "d:a t:150 u:a t:50").to_ascii();
    assert_eq!("t:100ms dn:Y t:50ms up:Y", result);
    let result = simulate(cfg, "d:a t:50 d:z t:75").to_ascii();
    assert_eq!("t:100ms dn:Y t:1ms dn:Z", result);
    let result = simulate(cfg, "d:a t:50 d:z u:z t:75").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms dn:Z t:1ms up:Z", result);
    let result = simulate(cfg, "d:a t:33 d:z t:33 d:v t:100").to_ascii();
    assert_eq!("t:66ms dn:X t:6ms dn:Z t:1ms dn:V", result);
    let result = simulate(cfg, "d:b t:33 d:z t:33 d:v t:100").to_ascii();
    assert_eq!("t:100ms dn:Y t:1ms dn:Z t:1ms dn:V", result);
    let result = simulate(cfg, "d:b t:20 d:z t:20 d:v t:20 d:w t:100").to_ascii();
    assert_eq!("t:60ms dn:X t:6ms dn:Z t:1ms dn:V t:1ms dn:W", result);
    let result = simulate(cfg, "d:c t:33 d:z t:33 d:v t:100").to_ascii();
    assert_eq!("t:100ms dn:Y t:1ms dn:Z t:1ms dn:V", result);
}

#[test]
fn tap_hold_release_keys() {
    let cfg = "
        (defsrc a)
        (deflayer l1 (tap-hold-release-keys 100 100 x y (z)))
    ";
    let result = simulate(cfg, "d:a t:20 u:a t:20 d:a t:200").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms up:X t:14ms dn:X", result);
    let result = simulate(cfg, "d:a t:20 u:a t:200 d:a t:200").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms up:X t:294ms dn:Y", result);
    let result = simulate(cfg, "d:a t:50 u:a t:50").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms up:X", result);
    let result = simulate(cfg, "d:a t:150 u:a t:50").to_ascii();
    assert_eq!("t:100ms dn:Y t:50ms up:Y", result);
    let result = simulate(cfg, "d:a t:50 d:z t:75").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms dn:Z", result);
    let result = simulate(cfg, "d:a t:50 d:z u:z t:75").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms dn:Z t:1ms up:Z", result);
}

#[test]
fn tap_hold_tap_keys() {
    let cfg = "
        (defsrc a b z)
        (deflayer l1 (tap-hold-tap-keys 100 100 x y (z)) b z)
    ";

    // Basic tap: release before timeout → tap action
    let result = simulate(cfg, "d:a t:50 u:a t:50").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms up:X", result);

    // Basic hold: timeout elapsed → hold action
    let result = simulate(cfg, "d:a t:150 u:a t:50").to_ascii();
    assert_eq!("t:100ms dn:Y t:50ms up:Y", result);

    // $tap-keys (z) pressed → immediate tap
    let result = simulate(cfg, "d:a t:50 d:z t:75").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms dn:Z", result);

    // KEY DIFFERENCE from tap-hold-release-keys:
    // Other key (b) pressed+released → wait for timeout, NOT immediate hold
    // This is the critical behavioral difference that tap-hold-tap-keys provides
    let result = simulate(cfg, "d:a t:50 d:b u:b t:100").to_ascii();
    // After 100ms timeout, hold activates, then b events are replayed
    assert_eq!("t:100ms dn:Y t:1ms dn:B t:49ms up:B", result);

    // Tap repress behavior
    let result = simulate(cfg, "d:a t:20 u:a t:20 d:a t:200").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms up:X t:14ms dn:X", result);
}
