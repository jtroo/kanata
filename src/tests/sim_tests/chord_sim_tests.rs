use super::*;

static SIMPLE_NONOVERLAPPING_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes) \
(defsrc) \
(defalias c c)
(defvar d d)
(deflayer base) \
(defchordsv2 \
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
        "t:50ms\nout:↓C\nt:102ms\nout:↑C\nt:98ms\nout:↓C\nt:102ms\nout:↑C",
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
        "t:50ms\nout:↓D\nt:52ms\nout:↑D\nt:148ms\nout:↓D\nt:52ms\nout:↑D",
        result
    );
}

static SIMPLE_OVERLAPPING_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes
 chords-v2-min-idle-experimental 5)
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
    assert_eq!("t:100ms\nout:↓C\nt:251ms\nout:↓Z\nt:51ms\nout:↑C", result);
}

#[test]
fn sim_presses_for_old_chord_repress_into_new_chord() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a d:b t:50 u:a t:50 d:z t:50 u:b t:50 d:a d:b t:50 u:a t:50",
    )
    .to_ascii();
    assert_eq!("t:50ms dn:C t:101ms up:C t:99ms dn:D t:11ms up:D", result);
}

#[test]
fn sim_chord_activate_largest_overlapping() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a t:50 d:b t:50 d:z t:50 d:y t:50 u:b t:50",
    );
    assert_eq!("t:150ms\nout:↓E\nt:52ms\nout:↑E", result);
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
(defchordsv2
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
(defchordsv2
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
        "t:199ms\nout:↓Y\nt:10ms\nout:↑Y\nt:193ms\nout:↓X\nt:10ms\nout:↑X",
        result
    );
}

static CHORD_WITH_PENDING_UNDERLYING_TAP_HOLD: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayermap (base) a (tap-hold 200 200 a b))
(defchordsv2
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
(defchordsv2
  (a b) _ 100 all-released ()
)";

#[test]
#[should_panic]
fn sim_denies_transparent() {
    simulate(CHORD_WITH_TRANSPARENCY, "");
}

#[test]
fn sim_chord_eager_tapholdpress_activation() {
    let result = simulate(
        "
    (defcfg concurrent-tap-hold yes)
    (defsrc caps j k bspc)
    (deflayer one (tap-hold-press 0 200 esc lctl) j k bspc)
    (defvirtualkeys bspc bspc)
    (defchordsv2
      (j k) (multi
        (on-press press-vkey bspc)
        (on-release release-vkey bspc)
        (fork XX bspc (nop9))) 75 first-release ()
    )
        ",
        "d:caps t:10 d:j d:k t:100 r:bspc t:10 r:bspc t:10 u:j u:k t:100 u:caps t:1000",
    )
    .to_ascii();
    assert_eq!(
        "t:11ms dn:LCtrl t:7ms dn:BSpace t:92ms \
         dn:BSpace t:10ms dn:BSpace t:14ms up:BSpace t:96ms up:LCtrl",
        result
    );
}

#[test]
fn sim_chord_eager_tapholdrelease_activation() {
    let result = simulate(
        "
    (defcfg concurrent-tap-hold yes)
    (defsrc caps j k bspc)
    (deflayer one (tap-hold-release 0 200 esc lctl) j k bspc)
    (defvirtualkeys bspc bspc)
    (defchordsv2
      (j k) (multi (on-press press-vkey bspc) (on-release release-vkey bspc)) 75 first-release ()
    )
        ",
        "d:caps t:10 d:j d:k t:10 u:j u:k t:100 u:caps t:1000",
    )
    .to_ascii();
    assert_eq!(
        "t:20ms dn:LCtrl t:7ms dn:BSpace t:5ms up:BSpace t:88ms up:LCtrl",
        result
    );
}

#[test]
fn sim_chord_release_nonchord_key_has_correct_order() {
    let result = simulate(
        "
    (defcfg concurrent-tap-hold yes)
    (defsrc ralt j k)
    (deflayer base _ _ _)
    (defchordsv2
      (j k) l 75 first-release ()
    )
        ",
        "d:ralt t:1000 d:j t:1 u:ralt t:100 u:j t:100",
    )
    .to_ascii();
    assert_eq!(
        "t:1ms dn:RAlt t:1075ms dn:J t:1ms up:RAlt t:24ms up:J",
        result
    );
}

#[test]
fn sim_chord_simultaneous_macro() {
    let result = simulate(
        "
        (defsrc a b o)
        (deflayer default
          (chord base a)
          (chord base b)
          (chord base o)
        )
        (defchords base 500
          (a) (macro a z)
          (b) (macro b)
          (o) o
          (a o) o
        )
        ",
        "d:a t:10 d:b t:500",
    )
    .to_ascii();
    assert_eq!(
        "t:502ms dn:A dn:B t:1ms up:A up:B t:1ms dn:Z t:1ms up:Z",
        result
    );
}

#[test]
#[should_panic]
fn sim_chord_error_on_duplicate_keyset() {
    simulate(
        "
(defcfg concurrent-tap-hold yes)
(defsrc)
(deflayer base)
(defchordsv2
 (1 2) (one-shot 2000 lsft) 20 all-released ()
 (2 1) (one-shot 2000 lctl) 20 all-released ()
)
        ",
        "",
    );
}

#[test]
fn sim_chord_oneshot() {
    let result = simulate(
        "
(defcfg concurrent-tap-hold yes)
(defsrc)(deflayer base)
(defchordsv2
  (a b) (one-shot 2500 rsft) 35 first-release ()
)
        ",
        "d:a t:10 d:b t:10 u:a t:10 u:b t:3000 \
         d:a t:10 d:b t:10 u:a t:10 u:b t:500 d:c u:c t:3000",
    )
    .to_ascii();
    assert_eq!(
        "t:10ms dn:RShift t:2500ms up:RShift t:530ms \
         dn:RShift t:521ms dn:C t:5ms up:RShift up:C",
        result
    );
}

#[test]
fn sim_chord_timeout_events() {
    let result = simulate(
        "
(defcfg
 concurrent-tap-hold yes
 process-unmapped-keys yes
)
(defvirtualkeys
 v-macro-word-end (macro spc)
)
(defsrc a b c)
(defchordsv2-experimental
 (a b c) (macro x y z (on-press tap-vkey v-macro-word-end)) 200 all-released ()
 (a b) (macro x y (on-press tap-vkey v-macro-word-end)) 200 all-released ()
)
(deflayer base a b c)
        ",
        "d:a t:10 d:b t:3000 u:a u:b t:100",
    )
    .to_ascii();
    assert_eq!(
        "t:201ms dn:X t:1ms up:X t:1ms dn:Y t:1ms up:Y t:4ms dn:Space t:1ms up:Space",
        result
    );
}
