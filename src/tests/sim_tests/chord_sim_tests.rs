use super::*;

static SIMPLE_NONOVERLAPPING_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes) \
(defsrc) \
(defalias c c)
(defvar d d)
(deflayer base) \
(defchordsv2-experimental \
  (a b) @c 200 all-released () \
  (b z) $d 200 first-release () \
)";

#[test]
fn sim_chord_basic_repeated_last_release() {
    let result = simulate(
        SIMPLE_NONOVERLAPPING_CHORD_CFG,
        "d:a t:50 d:b t:50 u:a t:50 u:b t:50 \
         d:b t:50 d:a t:50 u:b t:50 u:a t:50 ",
    );
    assert_eq!(
        "t:50ms\nout:↓C\nt:101ms\nout:↑C\nt:99ms\nout:↓C\nt:101ms\nout:↑C",
        result
    );
}

#[test]
fn sim_chord_min_idle_takes_effect() {
    let result = simulate(
        SIMPLE_NONOVERLAPPING_CHORD_CFG,
        "d:z t:20 d:a t:20 d:b t:20 d:d t:20",
    );
    assert_eq!(
        "t:21ms
out:↓Z
t:1ms
out:↓A
t:39ms
out:↓B
t:1ms
out:↓D",
        result
    );
}

#[test]
fn sim_timeout_hold_key() {
    let result = simulate(SIMPLE_NONOVERLAPPING_CHORD_CFG, "d:z t:201 d:b t:200");
    assert_eq!(
        "t:201ms
out:↓Z
t:1ms
out:↓B",
        result
    );
}

#[test]
fn sim_chord_basic_repeated_first_release() {
    let result = simulate(
        SIMPLE_NONOVERLAPPING_CHORD_CFG,
        "d:z t:50 d:b t:50 u:z t:50 u:b t:50 \
        d:z t:50 d:b t:50 u:z t:50 u:b t:50 ",
    );
    assert_eq!(
        "t:50ms\nout:↓D\nt:51ms\nout:↑D\nt:149ms\nout:↓D\nt:51ms\nout:↑D",
        result
    );
}

static SIMPLE_OVERLAPPING_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayer base)
(defchordsv2-experimental
  (a b) c 200 all-released ()
  (a b z) d 250 first-release ()
  (a b z y) e 400 first-release ()
)";

#[test]
fn sim_chord_overlapping_timeout() {
    let result = simulate(SIMPLE_OVERLAPPING_CHORD_CFG, "d:a d:b t:201 d:z t:300");
    assert_eq!(
        "t:200ms
out:↓C
t:252ms
out:↓Z",
        result
    );
}

#[test]
fn sim_chord_overlapping_release() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a d:b t:100 u:a d:z t:300 u:b t:300",
    );
    assert_eq!("t:100ms\nout:↓C\nt:251ms\nout:↓Z\nt:50ms\nout:↑C", result);
}

#[test]
fn sim_presses_for_old_chord_repress_into_new_chord() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a d:b t:50 u:a t:50 d:z t:50 u:b t:50 d:a d:b t:50 u:a t:50",
    );
    assert_eq!(
        "t:50ms\nout:↓C\nt:101ms\nout:↑C\nt:99ms\nout:↓D\nt:7ms\nout:↑D",
        result
    );
}

#[test]
fn sim_chord_activate_largest_overlapping() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a t:50 d:b t:50 d:z t:50 d:y t:50 u:b t:50",
    );
    assert_eq!("t:150ms\nout:↓E\nt:51ms\nout:↑E", result);
}

static SIMPLE_DISABLED_LAYER_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayermap (1) 2 (layer-switch 2)
                 3 (layer-switch 3))
(deflayermap (2) 3 (layer-while-held 3)
                 1 (layer-while-held 1))
(deflayermap (3) 2 (layer-while-held 2)
                 1 (layer-while-held 1))
(defchordsv2-experimental
  (a b) x 200 all-released (1)
  (c d) y 200 all-released (2)
  (e f) z 200 all-released (3)
)";

#[test]
fn sim_chord_layer_1_switch_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:1ms\nout:↓A\nt:50ms\nout:↓B\nt:99ms\nout:↓Y\nt:100ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_2_switch_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:2 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:100ms\nout:↓X\nt:51ms\nout:↓C\nt:50ms\nout:↓D\nt:99ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_3_switch_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:100ms\nout:↓X\nt:100ms\nout:↓Y\nt:51ms\nout:↓E\nt:50ms\nout:↓F",
        result
    );
}

#[test]
fn sim_chord_layer_1_held_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:1 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:101ms\nout:↓A\nt:50ms\nout:↓B\nt:99ms\nout:↓Y\nt:100ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_2_held_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:2 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:150ms\nout:↓X\nt:51ms\nout:↓C\nt:50ms\nout:↓D\nt:99ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_3_held_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:2 t:50 d:3 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:150ms\nout:↓X\nt:100ms\nout:↓Y\nt:51ms\nout:↓E\nt:50ms\nout:↓F",
        result
    );
}

#[test]
fn sim_chord_layer_3_repeat() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:a t:50 d:b t:50 r:b t:50 r:b t:50\n\
         d:d t:50 d:c t:50 r:c t:50 r:d t:50",
    );
    assert_eq!(
        "t:100ms\nout:↓X\nt:50ms\nout:↓X\nt:50ms\nout:↓X\n\
         t:100ms\nout:↓Y\nt:50ms\nout:↓Y\nt:50ms\nout:↓Y",
        result
    );
}

static CHORD_INTO_TAP_HOLD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayer base)
(defchordsv2-experimental
  (a b) (tap-hold 200 200 x y) 200 all-released ()
)";

#[test]
fn sim_chord_into_tap_hold() {
    let result = simulate(
        CHORD_INTO_TAP_HOLD_CFG,
        "d:a t:50 d:b t:149 u:a u:b t:5 \
         d:a t:50 d:b t:148 u:a u:b t:1000",
    );
    assert_eq!(
        "t:199ms\nout:↓Y\nt:3ms\nout:↑Y\nt:200ms\nout:↓X\nt:8ms\nout:↑X",
        result
    );
}

static CHORD_WITH_PENDING_UNDERLYING_TAP_HOLD: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayermap (base) a (tap-hold 200 200 a b))
(defchordsv2-experimental
  (b c) d 100 all-released ()
)";

#[test]
fn sim_chord_pending_tap_hold() {
    let result = simulate(
        CHORD_WITH_PENDING_UNDERLYING_TAP_HOLD,
        "d:a t:10 d:b t:10 d:c t:300",
    );
    // unlike other actions, chordv2 activations
    // are intentionally not delayed by waiting actions like tap-hold.
    assert_eq!("t:20ms\nout:↓D\nt:179ms\nout:↓B", result);
}

static CHORD_WITH_TRANSPARENCY: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayer base)
(defchordsv2-experimental
  (a b) _ 100 all-released ()
)";

#[test]
fn sim_denies_transparent() {
    Kanata::new_from_str(CHORD_WITH_TRANSPARENCY)
        .map(|_| ())
        .expect_err("trans in defchordsv2 should error");
}
