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
fn tap_hold_keys_hold_on_press() {
    let cfg = "
        (defsrc a b c)
        (deflayer l1
         (tap-hold-keys 100 100 x y (hold-on-press b))
         b c
        )
    ";
    // hold-on-press key triggers hold immediately
    let result = simulate(cfg, "d:a t:20 d:b t:100").to_ascii();
    assert_eq!("t:20ms dn:Y t:6ms dn:B", result);
    // non-listed key: PermissiveHold — needs press+release for hold
    let result = simulate(cfg, "d:a t:20 d:c t:20 u:c t:100").to_ascii();
    assert_eq!("t:40ms dn:Y t:6ms dn:C t:1ms up:C", result);
    // release before timeout with no other key → tap
    let result = simulate(cfg, "d:a t:50 u:a t:100").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms up:X", result);
    // timeout → hold
    let result = simulate(cfg, "d:a t:150 u:a t:100").to_ascii();
    assert_eq!("t:100ms dn:Y t:50ms up:Y", result);
}

#[test]
fn tap_hold_keys_tap_on_press() {
    let cfg = "
        (defsrc a b)
        (deflayer l1
         (tap-hold-keys 100 100 x y (tap-on-press b))
         b
        )
    ";
    // tap-on-press key triggers tap immediately
    let result = simulate(cfg, "d:a t:20 d:b t:100").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms dn:B", result);
}

#[test]
fn tap_hold_keys_tap_on_press_release() {
    let cfg = "
        (defsrc a b)
        (deflayer l1
         (tap-hold-keys 100 100 x y (tap-on-press-release b))
         b
        )
    ";
    // tap-on-press-release key needs press+release to trigger tap
    let result = simulate(cfg, "d:a t:20 d:b u:b t:100").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms dn:B t:1ms up:B", result);
    // tap-on-press-release key pressed but not released: no early tap or hold,
    // waits for timeout.
    let result = simulate(cfg, "d:a t:20 d:b t:200").to_ascii();
    assert_eq!("t:100ms dn:Y t:1ms dn:B", result);
}

#[test]
fn tap_hold_keys_all_lists() {
    let cfg = "
        (defsrc a b c d e)
        (deflayer l1
         (tap-hold-keys 100 100 x y
           (tap-on-press b)
           (tap-on-press-release c)
           (hold-on-press d))
         b c d e
        )
    ";
    // tap-on-press
    let result = simulate(cfg, "d:a t:20 d:b t:100").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms dn:B", result);
    // tap-on-press-release
    let result = simulate(cfg, "d:a t:20 d:c u:c t:100").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms dn:C t:1ms up:C", result);
    // hold-on-press
    let result = simulate(cfg, "d:a t:20 d:d t:100").to_ascii();
    assert_eq!("t:20ms dn:Y t:6ms dn:D", result);
    // unlisted key: PermissiveHold
    let result = simulate(cfg, "d:a t:20 d:e t:20 u:e t:100").to_ascii();
    assert_eq!("t:40ms dn:Y t:6ms dn:E t:1ms up:E", result);
}

#[test]
fn tap_hold_keys_with_require_prior_idle() {
    let cfg = "
        (defsrc a b)
        (deflayer l1
         (tap-hold-keys 100 100 x y
           (hold-on-press b)
           (require-prior-idle 200))
         b
        )
    ";
    // Quick typing should resolve as tap due to require-prior-idle
    let result = simulate(cfg, "d:b t:10 u:b t:10 d:a t:50 u:a t:100").to_ascii();
    assert_eq!("dn:B t:10ms up:B t:10ms dn:X t:50ms up:X", result);
}

#[test]
fn tap_hold_keys_with_multiple_mods_require_prior_idle() {
    let cfg = "
        (defsrc)
        (deflayermap (base)
         a (tap-hold-keys 100 100 a b
           (require-prior-idle 200))
         b (tap-hold-keys 100 100 m n
           (require-prior-idle 200))
         c (tap-hold-keys 100 100 x y
           (require-prior-idle 200))
        )
    ";
    // Quick typing should resolve as tap due to require-prior-idle
    let result = simulate(cfg, "d:a t:10 d:b t:10 d:c t:310").to_ascii();
    assert_eq!("t:100ms dn:B t:101ms dn:N t:101ms dn:Y", result);
}

#[test]
fn tap_hold_keys_with_multiple_mods_require_prior_idle_concerrent() {
    let cfg = "
        (defcfg concurrent-tap-hold yes)
        (defsrc)
        (deflayermap (base)
         a (tap-hold-keys 100 100 a b
           (require-prior-idle 200))
         b (tap-hold-keys 100 100 m n
           (require-prior-idle 200))
         c (tap-hold-keys 100 100 x y
           (require-prior-idle 200))
        )
    ";
    // Quick typing should resolve as tap due to require-prior-idle
    let result = simulate(cfg, "d:a t:10 d:b t:10 d:c t:130").to_ascii();
    assert_eq!("t:99ms dn:B t:10ms dn:N t:10ms dn:Y", result);
}

#[test]
fn tap_hold_keys_no_options() {
    // With no key list options, tap-hold-keys behaves like tap-hold-release (PermissiveHold).
    let cfg = "
        (defsrc a b)
        (deflayer l1
         (tap-hold-keys 100 100 x y)
         b
        )
    ";
    // press+release of another key triggers hold
    let result = simulate(cfg, "d:a t:20 d:b t:20 u:b t:100").to_ascii();
    assert_eq!("t:40ms dn:Y t:6ms dn:B t:1ms up:B", result);
    // release before timeout → tap
    let result = simulate(cfg, "d:a t:50 u:a t:100").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms up:X", result);
    // timeout → hold
    let result = simulate(cfg, "d:a t:150 u:a t:100").to_ascii();
    assert_eq!("t:100ms dn:Y t:50ms up:Y", result);
}

#[test]
fn tap_hold_keys_duplicate_key_across_lists() {
    // A key appearing in multiple lists should be rejected at parse time.
    let cfg = "
        (defsrc a b)
        (deflayer l1
         (tap-hold-keys 100 100 x y
           (tap-on-press b)
           (hold-on-press b))
         b
        )
    ";
    let result = Kanata::new_from_str(cfg, Default::default());
    assert!(
        result.is_err(),
        "expected error for duplicate key across tap-on-press and hold-on-press"
    );

    let cfg2 = "
        (defsrc a b)
        (deflayer l1
         (tap-hold-keys 100 100 x y
           (tap-on-press-release b)
           (hold-on-press b))
         b
        )
    ";
    let result = Kanata::new_from_str(cfg2, Default::default());
    assert!(
        result.is_err(),
        "expected error for duplicate key across tap-on-press-release and hold-on-press"
    );
}

#[test]
fn tap_hold_opposite_hand_release_basic() {
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a j k)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y (same-hand tap) (unknown-hand hold))
         j k
        )
    ";
    // Opposite-hand key pressed + released → hold
    let result = simulate(cfg, "d:a t:20 d:j t:20 u:j t:100").to_ascii();
    assert_eq!("t:40ms dn:Y t:6ms dn:J t:1ms up:J", result);
}

#[test]
fn tap_hold_opposite_hand_release_same_hand_tap() {
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a s j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y (same-hand tap) (unknown-hand hold))
         s j
        )
    ";
    // Same-hand key pressed + released → tap (resolves on press, not release)
    let result = simulate(cfg, "d:a t:20 d:s t:20 u:s t:100").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms dn:S t:14ms up:S", result);

    // Same-hand key pressed but NOT released → tap immediately on press
    // (same-hand doesn't require release, only opposite-hand does)
    let result = simulate(cfg, "d:a t:20 d:s t:250").to_ascii();
    assert_eq!("t:20ms dn:X t:6ms dn:S", result);
}

#[test]
fn tap_hold_opposite_hand_release_no_interrupt() {
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y (same-hand tap) (unknown-hand hold))
         j
        )
    ";
    // Released before timeout, no interrupt → tap
    let result = simulate(cfg, "d:a t:50 u:a t:100").to_ascii();
    assert_eq!("t:50ms dn:X t:6ms up:X", result);

    // Timeout, no interrupt → timeout action (tap by default)
    let result = simulate(cfg, "d:a t:250 u:a t:100").to_ascii();
    assert_eq!("t:200ms dn:X t:50ms up:X", result);

    // With (timeout hold), timeout → hold
    let cfg_timeout_hold = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y (same-hand tap) (unknown-hand hold) (timeout hold))
         j
        )
    ";
    let result = simulate(cfg_timeout_hold, "d:a t:250 u:a t:100").to_ascii();
    assert_eq!("t:200ms dn:Y t:50ms up:Y", result);
}

#[test]
fn tap_hold_opposite_hand_release_vs_press() {
    // Compare: press-time (existing) vs release-time (new)
    // With press-time, opposite-hand key triggers hold immediately on press.
    // With release-time, it waits for the key's release.
    let cfg_press = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a j)
        (deflayer l1
         (tap-hold-opposite-hand 200 x y (same-hand tap) (unknown-hand hold))
         j
        )
    ";
    let cfg_release = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y (same-hand tap) (unknown-hand hold))
         j
        )
    ";
    // Press-time: hold decides on press (before j is released)
    let result = simulate(cfg_press, "d:a t:20 d:j t:100").to_ascii();
    assert_eq!("t:20ms dn:Y t:6ms dn:J", result);

    // Release-time: does NOT decide on press alone — waits for release
    let result = simulate(cfg_release, "d:a t:20 d:j t:20 u:j t:100").to_ascii();
    assert_eq!("t:40ms dn:Y t:6ms dn:J t:1ms up:J", result);
}

#[test]
fn tap_hold_opposite_hand_release_with_require_prior_idle() {
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a s j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y
           (same-hand tap) (unknown-hand hold) (require-prior-idle 200))
         s j
        )
    ";
    // Quick typing should resolve as tap due to require-prior-idle
    let result = simulate(cfg, "d:s t:10 u:s t:10 d:a t:50 u:a t:100").to_ascii();
    assert_eq!("dn:S t:10ms up:S t:10ms dn:X t:50ms up:X", result);
}

#[test]
fn tap_hold_opposite_hand_release_same_hand_hold() {
    // (same-hand hold) should trigger hold immediately on press (no release needed)
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a s j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y (same-hand hold) (unknown-hand hold))
         s j
        )
    ";
    let result = simulate(cfg, "d:a t:20 d:s t:20 u:s t:100").to_ascii();
    assert_eq!("t:20ms dn:Y t:6ms dn:S t:14ms up:S", result);
}

#[test]
fn tap_hold_opposite_hand_release_same_hand_then_opposite() {
    // Regression test: pressing two same-hand HRM keys together then an
    // opposite-hand key should resolve the first as tap (via same-hand),
    // not as hold (via opposite-hand release).
    // Bug: the -release variant required ALL keys to have press+release before
    // considering them, so same-hand keys were skipped if still held, and the
    // opposite-hand key's release incorrectly triggered Hold.
    let cfg = "
        (defcfg concurrent-tap-hold yes process-unmapped-keys yes)
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc d f j)
        (deflayer l1
         (tap-hold-opposite-hand-release 500 d lsft (same-hand tap) (timeout tap))
         (tap-hold-opposite-hand-release 500 f lctl (same-hand tap) (timeout tap))
         j
        )
    ";
    // f↓ d↓ j↓ j↑ → f sees d (same-hand) → tap immediately; d sees j (opposite+release) → hold
    // Result: f (tap) at t=5, then lsft+j when j is released at t=45
    let result = simulate(cfg, "d:f t:5 d:d t:20 d:j t:20 u:j t:100").to_ascii();
    assert_eq!("t:5ms dn:F t:40ms dn:LShift t:6ms dn:J t:1ms up:J", result);
}

#[test]
fn tap_hold_opposite_hand_release_same_hand_ignore() {
    // (same-hand ignore) should skip same-hand keys and wait for timeout
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a s j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y
           (same-hand ignore) (unknown-hand hold) (timeout hold))
         s j
        )
    ";
    // Same-hand key pressed+released is ignored, timeout fires
    let result = simulate(cfg, "d:a t:20 d:s t:20 u:s t:250").to_ascii();
    assert_eq!("t:200ms dn:Y t:1ms dn:S t:1ms up:S", result);
}

#[test]
fn tap_hold_opposite_hand_release_neutral_keys() {
    let cfg = "
        (defhands
          (left  a s d f)
          (right j k l ;))
        (defsrc a spc j)
        (deflayer l1
         (tap-hold-opposite-hand-release 200 x y
           (same-hand tap) (unknown-hand hold) (neutral-keys spc) (neutral tap))
         spc j
        )
    ";
    // spc is in neutral-keys with (neutral tap) → tap on press+release
    let result = simulate(cfg, "d:a t:20 d:spc t:20 u:spc t:100").to_ascii();
    assert_eq!("t:40ms dn:X t:6ms dn:Space t:1ms up:Space", result);
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
      (left a s d f g)
      (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
        (defsrc f h spc)
        (deflayer base @f h spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys spc)))
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
        (defhands (left a s d f g) (right h j k l ;))
        (defsrc f h spc)
        (deflayer base @f h spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys spc) (neutral tap)))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
        (defsrc d f j spc)
        (deflayer base d @f j spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl
          (same-hand hold) (timeout hold)
          (neutral-keys spc) (neutral tap)))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
        (defsrc f h spc)
        (deflayer base @f h spc)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys spc) (neutral hold)))
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
        (defhands (left a s d f g) (right h j k l ;))
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
        (defhands (left a s d f g) (right h j k l ;))
        (defsrc f j)
        (deflayer base @f j)
        (defalias f (tap-hold-opposite-hand 200 f lctl (neutral-keys j) (neutral tap)))
        ",
        "d:f t:50 d:j t:50 u:j t:50 u:f t:50",
    )
    .to_ascii();
    // j would normally be opposite-hand (hold), but neutral-keys overrides -> tap
    assert_eq!("t:50ms dn:F t:6ms dn:J t:44ms up:J t:50ms up:F", result);
}

// ========== tap-hold-require-prior-idle tests ==========

#[test]
fn tap_hold_require_prior_idle_typing_streak_resolves_tap() {
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a b d)
(deflayer base a b @d)
(defalias d (tap-hold 200 200 d lctl))
        ",
        // Type a, release, then quickly press d within idle window.
        // 'a' was pressed 20ms ago (10ms press + 10ms gap), well within 150ms threshold.
        "d:a t:10 u:a t:10 d:d t:50 u:d t:50",
    )
    .to_ascii();
    // d should resolve as tap immediately (no 200ms waiting state)
    assert_eq!("dn:A t:10ms up:A t:10ms dn:D t:50ms up:D", result);
}

#[test]
fn tap_hold_require_prior_idle_idle_long_enough_enters_hold() {
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a b d)
(deflayer base a b @d)
(defalias d (tap-hold 200 200 d lctl))
        ",
        // Press a, release, wait 200ms (longer than 150ms threshold), then press d.
        // d should enter normal WaitingState (hold on other key press).
        "d:a t:10 u:a t:200 d:d t:250 u:d t:50",
    )
    .to_ascii();
    // After 200ms idle, d enters normal tap-hold. Timeout at 200ms → hold (lctl).
    assert_eq!("dn:A t:10ms up:A t:400ms dn:LCtrl t:50ms up:LCtrl", result);
}

#[test]
fn tap_hold_require_prior_idle_no_prior_key_enters_hold() {
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a b d)
(deflayer base a b @d)
(defalias d (tap-hold 200 200 d lctl))
        ",
        // No prior key at all. d should enter normal WaitingState.
        "d:d t:250 u:d t:50",
    )
    .to_ascii();
    // Timeout → hold (lctl)
    assert_eq!("t:200ms dn:LCtrl t:50ms up:LCtrl", result);
}

#[test]
fn tap_hold_require_prior_idle_boundary_just_within_threshold() {
    // Prior key pressed 149ms ago (just within 150ms threshold).
    // ticks_since_occurrence will be ~150 (149 + 1 tick offset), which is
    // <= 150 threshold, so tap fires.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold 200 200 d lctl))
        ",
        "d:a t:10 u:a t:139 d:d t:50 u:d t:50",
    )
    .to_ascii();
    assert_eq!("dn:A t:10ms up:A t:139ms dn:D t:50ms up:D", result);
}

#[test]
fn tap_hold_require_prior_idle_boundary_just_outside_threshold() {
    // Prior key pressed 150ms ago (just outside 150ms threshold).
    // ticks_since_occurrence will be ~151 (150 + 1 tick offset), which is
    // > 150 threshold, so normal tap-hold behavior applies.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold 200 200 d lctl))
        ",
        // d enters WaitingState; released at 50ms → tap via release
        "d:a t:10 u:a t:140 d:d t:50 u:d t:50",
    )
    .to_ascii();
    assert_eq!("dn:A t:10ms up:A t:190ms dn:D t:6ms up:D", result);
}

#[test]
fn tap_hold_require_prior_idle_with_opposite_hand() {
    // tap-hold-require-prior-idle should short-circuit before tap-hold-opposite-hand
    // evaluates hand membership. During a typing streak, even an opposite-hand
    // key should resolve as tap.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defhands (left a s d f g) (right h j k l ;))
(defsrc a f j)
(deflayer base a @f j)
(defalias f (tap-hold-opposite-hand 200 f lctl))
        ",
        // a is left hand, f is left hand tap-hold. a pressed 20ms ago.
        // Without tap-hold-require-prior-idle, pressing j (opposite hand) would hold.
        // With tap-hold-require-prior-idle active, f resolves as tap before hand check.
        "d:a t:10 u:a t:10 d:f t:50 u:f t:50",
    )
    .to_ascii();
    assert_eq!("dn:A t:10ms up:A t:10ms dn:F t:50ms up:F", result);
}

#[test]
fn tap_hold_require_prior_idle_with_tap_hold_interval() {
    // tap-hold-require-prior-idle check runs before tap-hold-interval (quick re-press).
    // Both should work together: typing streak → tap immediately,
    // idle re-press → tap via tap_hold_interval.
    let cfg = "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold 200 200 d lctl))
    ";
    // Case 1: typing streak (a then d quickly) → tap-hold-require-prior-idle fires
    let result = simulate(cfg, "d:a t:10 u:a t:10 d:d t:50 u:d t:50").to_ascii();
    assert_eq!("dn:A t:10ms up:A t:10ms dn:D t:50ms up:D", result);
    // Case 2: idle, then d pressed twice (tap-hold-interval re-press)
    let result = simulate(cfg, "d:d t:50 u:d t:50 d:d t:50 u:d t:50").to_ascii();
    assert_eq!("t:50ms dn:D t:6ms up:D t:44ms dn:D t:50ms up:D", result);
}

#[test]
fn tap_hold_require_prior_idle_ignores_virtual_keys() {
    // Virtual key events (row 1) should not count as prior physical input.
    // Only real physical key presses (row 0) trigger the typing streak.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc d)
(defvirtualkeys vk1 a)
(deflayer base @d)
(defalias d (tap-hold 200 200 d lctl))
        ",
        // Virtual key tap, then d pressed 10ms later.
        // vk should NOT trigger typing streak — d should enter normal hold.
        "vk:vk1:tap t:10 d:d t:250 u:d t:50",
    )
    .to_ascii();
    // Virtual key outputs A, then d times out to hold (lctl).
    assert_eq!("dn:A t:1ms up:A t:209ms dn:LCtrl t:50ms up:LCtrl", result);
}

// ========== per-action require-prior-idle override tests ==========

#[test]
fn per_action_require_prior_idle_overrides_global() {
    // Global defcfg sets 150ms, but per-action override sets 50ms.
    // A prior key 60ms ago is within 150ms (global) but outside 50ms (per-action).
    // Per-action should win: normal hold behavior, not tap.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold 200 200 d lctl (require-prior-idle 50)))
        ",
        "d:a t:10 u:a t:50 d:d t:250 u:d t:50",
    )
    .to_ascii();
    // 60ms gap > 50ms per-action threshold → normal hold
    assert_eq!("dn:A t:10ms up:A t:250ms dn:LCtrl t:50ms up:LCtrl", result);
}

#[test]
fn per_action_require_prior_idle_disable_overrides_global() {
    // Global defcfg sets 150ms, but per-action override sets 0 (disabled).
    // Even during a typing streak, this action should use normal tap-hold.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold 200 200 d lctl (require-prior-idle 0)))
        ",
        // a pressed 20ms ago, well within 150ms global threshold.
        // But per-action disables it, so d enters normal WaitingState.
        "d:a t:10 u:a t:10 d:d t:250 u:d t:50",
    )
    .to_ascii();
    // d enters hold (250ms > 200ms timeout)
    assert_eq!("dn:A t:10ms up:A t:210ms dn:LCtrl t:50ms up:LCtrl", result);
}

#[test]
fn per_action_require_prior_idle_enables_without_global() {
    // No global defcfg (default 0), but per-action sets 150ms.
    // The per-action value should enable the feature for this action only.
    let result = simulate(
        "
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold 200 200 d lctl (require-prior-idle 150)))
        ",
        "d:a t:10 u:a t:10 d:d t:50 u:d t:50",
    )
    .to_ascii();
    // a pressed 20ms ago, within 150ms per-action threshold → tap
    assert_eq!("dn:A t:10ms up:A t:10ms dn:D t:50ms up:D", result);
}

#[test]
fn per_action_require_prior_idle_mixed_actions() {
    // The issue #1967 use case: two tap-hold keys with different idle behavior.
    // @a (HRM) uses the global 150ms threshold.
    // @d (layer key) disables idle detection via per-action override.
    // During a typing streak, @a should resolve as tap but @d should hold.
    let cfg = "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d e)
(deflayer base @a @d e)
(defalias
  a (tap-hold-press 200 200 a lmet)
  d (tap-hold-press 200 200 d lctl (require-prior-idle 0))
)
    ";
    // Case 1: type e, then quickly press @a → global idle fires, a resolves as tap
    let result = simulate(cfg, "d:e t:10 u:e t:10 d:a t:50 u:a t:50").to_ascii();
    assert_eq!("dn:E t:10ms up:E t:10ms dn:A t:50ms up:A", result);
    // Case 2: type e, then quickly press @d → per-action 0 disables idle, d enters hold.
    // tap-hold-press resolves on next key press: e pressed 10ms after d triggers hold.
    let result = simulate(cfg, "d:e t:10 u:e t:10 d:d t:10 d:e t:50 u:e t:50 u:d t:50").to_ascii();
    assert_eq!(
        "dn:E t:10ms up:E t:20ms dn:LCtrl t:6ms dn:E t:44ms up:E t:50ms up:LCtrl",
        result
    );
}

#[test]
fn per_action_require_prior_idle_with_tap_hold_release() {
    // Per-action option works with tap-hold-release variant.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold-release 200 200 d lctl (require-prior-idle 0)))
        ",
        // Typing streak: a pressed 20ms ago. Global would force tap,
        // but per-action 0 disables it.
        "d:a t:10 u:a t:10 d:d t:250 u:d t:50",
    )
    .to_ascii();
    // d enters normal hold (per-action override disables idle check)
    assert_eq!("dn:A t:10ms up:A t:210ms dn:LCtrl t:50ms up:LCtrl", result);
}

#[test]
fn per_action_require_prior_idle_with_opposite_hand() {
    // Per-action option works with tap-hold-opposite-hand variant.
    // Use j (right hand) pressing before f (left hand tap-hold).
    // Without require-prior-idle, j is opposite-hand → f would hold.
    // With global require-prior-idle 150, the typing streak forces tap.
    // With per-action override 0, the override disables it and f holds.
    let result = simulate(
        "
(defcfg tap-hold-require-prior-idle 150)
(defhands (left a s d f g) (right h j k l ;))
(defsrc j f)
(deflayer base j @f)
(defalias f (tap-hold-opposite-hand 200 f lctl
  (timeout hold)
  (require-prior-idle 0)))
        ",
        // j (right) pressed 20ms ago, then f (left) pressed.
        // Global 150ms would force tap, but per-action 0 disables it.
        // j is opposite hand → f should hold. timeout → hold.
        "d:j t:10 u:j t:10 d:f t:250 u:f t:50",
    )
    .to_ascii();
    // f enters normal opposite-hand behavior: j is opposite → hold
    assert_eq!("dn:J t:10ms up:J t:210ms dn:LCtrl t:50ms up:LCtrl", result);
}

// ========== tap-hold-order simulation tests ==========
// Note: t:6ms gaps after resolution are sim framework processing overhead for
// event-triggered resolution (as opposed to timeout-triggered). This is consistent
// with other event-driven tap-hold variants (e.g., tap-hold-opposite-hand).

#[test]
fn tap_hold_order_clean_tap() {
    // Press and release tap-hold-order key with no other keys → Tap.
    let result = simulate(
        "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-order 200 50 a lctl))
        ",
        "d:a t:100 u:a t:50",
    )
    .to_ascii();
    assert_eq!("t:100ms dn:A t:6ms up:A", result);
}

#[test]
fn tap_hold_order_hold_other_released_first() {
    // TH down → other down → other up (released first) → Hold.
    let result = simulate(
        "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-order 200 0 a lctl))
        ",
        "d:a t:10 d:b t:10 u:b t:10 u:a t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:20ms dn:LCtrl t:6ms dn:B t:1ms up:B t:3ms up:LCtrl",
        result
    );
}

#[test]
fn tap_hold_order_tap_modifier_released_first() {
    // TH down → other down → TH up first → Tap.
    let result = simulate(
        "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-order 200 0 a lctl))
        ",
        "d:a t:10 d:b t:10 u:a t:10 u:b t:50",
    )
    .to_ascii();
    assert_eq!("t:20ms dn:A t:6ms dn:B t:1ms up:A t:3ms up:B", result);
}

#[test]
fn tap_hold_order_buffer_ignores_fast_typing() {
    // Other key pressed+released within buffer window → ignored by
    // release-order logic. TH released → Tap.
    let result = simulate(
        "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-order 200 50 a lctl))
        ",
        "d:a t:10 d:b t:10 u:b t:10 u:a t:50",
    )
    .to_ascii();
    // Without buffer, b's press+release would trigger Hold.
    // With buffer=50, b's press at 10ms is within window → ignored → Tap.
    assert_eq!("t:30ms dn:A t:6ms dn:B t:1ms up:B t:1ms up:A", result);
}

#[test]
fn tap_hold_order_hold_after_buffer_expires() {
    // Other key pressed after buffer window expires → release-order applies.
    // Other released first → Hold.
    let result = simulate(
        "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-order 200 50 a lctl))
        ",
        "d:a t:60 d:b t:10 u:b t:10 u:a t:50",
    )
    .to_ascii();
    // b pressed at 60ms (after 50ms buffer) → release-order active.
    // b released first → Hold.
    assert_eq!(
        "t:70ms dn:LCtrl t:6ms dn:B t:1ms up:B t:3ms up:LCtrl",
        result
    );
}

#[test]
fn tap_hold_order_with_require_prior_idle() {
    // Per-action require-prior-idle short-circuits to tap during typing streak.
    let result = simulate(
        "
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold-order 200 50 d lctl (require-prior-idle 150)))
        ",
        // a pressed 20ms ago → within 150ms idle threshold → tap immediately.
        "d:a t:10 u:a t:10 d:d t:50 u:d t:50",
    )
    .to_ascii();
    assert_eq!("dn:A t:10ms up:A t:10ms dn:D t:50ms up:D", result);
}

#[test]
fn tap_hold_order_no_prior_idle_enters_normal_resolution() {
    // No recent keypress → require-prior-idle doesn't fire → normal release-order.
    // Other key released first → Hold.
    let result = simulate(
        "
(defsrc a d)
(deflayer base a @d)
(defalias d (tap-hold-order 200 0 d lctl (require-prior-idle 150)))
        ",
        "d:d t:10 d:a t:10 u:a t:10 u:d t:50",
    )
    .to_ascii();
    assert_eq!(
        "t:20ms dn:LCtrl t:6ms dn:A t:1ms up:A t:3ms up:LCtrl",
        result
    );
}
