use super::*;

#[test]
fn delayed_timedout_released_taphold_can_still_tap() {
    let result = simulate(
        "
        (defcfg concurrent-tap-hold yes )
        (defsrc a b j)
        (deflayer base @a @b @j)
        (defalias
         a (tap-hold 200 1000 a lctl)
         b (tap-hold-tap-keys 0 100 b c (j))
         j (tap-hold 200 500 j lsft))
        ",
        "d:a t:50 d:b t:50 d:j t:10 u:j t:100 u:b t:100 u:a t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:310ms dn:A t:7ms dn:B t:7ms dn:J t:6ms up:J t:1ms up:B t:1ms up:A",
        result
    );
}

#[test]
fn delayed_timedout_released_taphold_can_hold() {
    let result = simulate(
        "
        (defcfg concurrent-tap-hold yes)
        (defsrc a b)
        (deflayer base @a @b)
        (defalias
          a (tap-hold 0 300 a b)
          b (tap-hold 0 100 c d)
        )
        ",
        "d:a t:50 d:b t:150 u:b t:50 u:a t:50",
    )
    .to_ascii();
    assert_eq!("t:250ms dn:A t:7ms dn:D t:1ms up:D t:1ms up:A", result);
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
    // d:b and u:b have no delay between them, so both replay with minimal gap
    assert_eq!("t:100ms dn:Y t:1ms dn:B t:1ms up:B", result);

    // Tap repress behavior
    let result = simulate(cfg, "d:a t:20 u:a t:20 d:a t:200").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms up:X t:14ms dn:X", result);
}

// ========== tap-hold-opposite-hand tests ==========

fn opposite_hand_cfg() -> &'static str {
    "
    (defhands
      left (a s d f g)
      right (h j k l ;))
    (defsrc f j h)
    (deflayer base @f j h)
    (defalias
      f (tap-hold-opposite-hand 200 f lctl))
    "
}

#[test]
fn opposite_hand_press_resolves_hold() {
    // Press f (left hand), then j (right hand) -> should resolve as HOLD (lctl)
    let result = simulate(opposite_hand_cfg(), "d:f t:50 d:j t:50 u:j t:50 u:f t:50").to_ascii();
    assert_eq!(
        "t:50ms dn:LCtrl t:6ms dn:J t:44ms up:J t:50ms up:LCtrl",
        result
    );
}

#[test]
fn same_hand_press_resolves_tap() {
    // Press f (left hand), then d (left hand) -> same hand default = tap
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc d f)
        (deflayer base d @f)
        (defalias f (tap-hold-opposite-hand 200 f lctl))
        ",
        "d:f t:50 d:d t:50 u:d t:50 u:f t:50",
    )
    .to_ascii();
    assert_eq!("t:50ms dn:F t:6ms dn:D t:44ms up:D t:50ms up:F", result);
}

#[test]
fn same_hand_ignore_defers_to_timeout() {
    // With (same-hand ignore), a same-hand press is ignored, timeout fires
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc d f)
        (deflayer base d @f)
        (defalias f (tap-hold-opposite-hand 200 f lctl (same-hand ignore)))
        ",
        "d:f t:50 d:d t:200 u:d t:50 u:f t:50",
    )
    .to_ascii();
    // timeout (default=tap) fires at 200ms from f press
    assert_eq!("t:200ms dn:F t:1ms dn:D t:49ms up:D t:50ms up:F", result);
}

#[test]
fn neutral_key_ignore_defers() {
    // With default (neutral ignore), neutral keys are skipped, timeout fires
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f h spc)
        (deflayer base @f h spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys (spc))))
        ",
        "d:f t:50 d:spc t:200 u:spc t:50 u:f t:50",
    )
    .to_ascii();
    // spc is neutral, default (neutral ignore), so timeout fires
    assert_eq!(
        "t:200ms dn:F t:1ms dn:Space t:49ms up:Space t:50ms up:F",
        result
    );
}

#[test]
fn neutral_key_tap_resolves_immediately() {
    // With (neutral tap), neutral key press triggers TAP immediately
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f h spc)
        (deflayer base @f h spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys (spc)) (neutral tap)))
        ",
        "d:f t:50 d:spc t:50 u:spc t:50 u:f t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:50ms dn:F t:6ms dn:Space t:44ms up:Space t:50ms up:F",
        result
    );
}

#[test]
fn unknown_hand_key_defers_by_default() {
    // Key not in defhands at all -> unknown hand, default = ignore
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f b)
        (deflayer base @f b)
        (defalias f (tap-hold-opposite-hand 200 f lctl))
        ",
        "d:f t:50 d:b t:200 u:b t:50 u:f t:50",
    )
    .to_ascii();
    // b is not in defhands, unknown-hand default = ignore, timeout fires
    assert_eq!("t:200ms dn:F t:1ms dn:B t:49ms up:B t:50ms up:F", result);
}

#[test]
fn timeout_default_is_tap() {
    // Default timeout behavior is tap
    let result = simulate(opposite_hand_cfg(), "d:f t:250 u:f t:50").to_ascii();
    assert_eq!("t:200ms dn:F t:50ms up:F", result);
}

#[test]
fn timeout_hold_option() {
    // (timeout hold) makes timeout resolve to hold action
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f j)
        (deflayer base @f j)
        (defalias f (tap-hold-opposite-hand 200 f lctl (timeout hold)))
        ",
        "d:f t:250 u:f t:50",
    )
    .to_ascii();
    assert_eq!("t:200ms dn:LCtrl t:50ms up:LCtrl", result);
}

#[test]
fn release_before_timeout_taps() {
    // Release the hold-tap key before timeout -> immediate tap
    let result = simulate(opposite_hand_cfg(), "d:f t:50 u:f t:50").to_ascii();
    assert_eq!("t:50ms dn:F t:6ms up:F", result);
}

#[test]
fn multiple_options_combined() {
    // Combine (same-hand hold), (timeout hold), (neutral-keys ...) with (neutral tap)
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc d f j spc)
        (deflayer base d @f j spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl
          (same-hand hold) (timeout hold)
          (neutral-keys (spc)) (neutral tap)))
        ",
        "d:f t:50 d:d t:50 u:d t:50 u:f t:50",
    )
    .to_ascii();
    // d is same hand, (same-hand hold) -> resolves as hold
    assert_eq!(
        "t:50ms dn:LCtrl t:6ms dn:D t:44ms up:D t:50ms up:LCtrl",
        result
    );
}

#[test]
fn unknown_hand_tap_resolves_immediately() {
    // (unknown-hand tap) makes unassigned keys resolve as tap
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f b)
        (deflayer base @f b)
        (defalias f (tap-hold-opposite-hand 200 f lctl (unknown-hand tap)))
        ",
        "d:f t:50 d:b t:50 u:b t:50 u:f t:50",
    )
    .to_ascii();
    assert_eq!("t:50ms dn:F t:6ms dn:B t:44ms up:B t:50ms up:F", result);
}

#[test]
fn unknown_hand_hold_resolves_immediately() {
    // (unknown-hand hold) makes unassigned keys resolve as hold
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f b)
        (deflayer base @f b)
        (defalias f (tap-hold-opposite-hand 200 f lctl (unknown-hand hold)))
        ",
        "d:f t:50 d:b t:50 u:b t:50 u:f t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:50ms dn:LCtrl t:6ms dn:B t:44ms up:B t:50ms up:LCtrl",
        result
    );
}

#[test]
fn neutral_key_hold_resolves_immediately() {
    // (neutral hold) makes neutral keys resolve as hold
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f h spc)
        (deflayer base @f h spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys (spc)) (neutral hold)))
        ",
        "d:f t:50 d:spc t:50 u:spc t:50 u:f t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:50ms dn:LCtrl t:6ms dn:Space t:44ms up:Space t:50ms up:LCtrl",
        result
    );
}

#[test]
fn waiting_key_unassigned_in_defhands() {
    // The hold-tap key (b) is NOT in defhands, so its hand is unknown.
    // Pressing j (right hand) still triggers unknown-hand logic (both sides unknown = unknown).
    // Default (unknown-hand ignore), so it defers; timeout fires as tap.
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc b j)
        (deflayer base @b j)
        (defalias b (tap-hold-opposite-hand 200 b lctl))
        ",
        "d:b t:50 d:j t:200 u:j t:50 u:b t:50",
    )
    .to_ascii();
    assert_eq!("t:200ms dn:B t:1ms dn:J t:49ms up:J t:50ms up:B", result);
}

#[test]
fn neutral_keys_override_defhands_assignment() {
    // j is in defhands (right hand), but also in (neutral-keys ...).
    // (neutral-keys ...) takes precedence, so j is treated as neutral.
    // With (neutral tap), pressing j should resolve as tap (not hold).
    let result = simulate(
        "
        (defhands left (a s d f g) right (h j k l ;))
        (defsrc f j)
        (deflayer base @f j)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys (j)) (neutral tap)))
        ",
        "d:f t:50 d:j t:50 u:j t:50 u:f t:50",
    )
    .to_ascii();
    // j would normally be opposite-hand (hold), but neutral-keys overrides -> tap
    assert_eq!("t:50ms dn:F t:6ms dn:J t:44ms up:J t:50ms up:F", result);
}
