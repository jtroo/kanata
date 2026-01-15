use super::*;

#[test]
fn transparent_base() {
    let result = simulate(
        "(defcfg process-unmapped-keys yes concurrent-tap-hold yes) \
         (defsrc a) \
         (deflayer base _)",
        "d:a t:50 u:a t:50",
    );
    assert_eq!("out:↓A\nt:50ms\nout:↑A", result);
}

#[test]
fn delegate_base() {
    let result = simulate(
        "(defcfg process-unmapped-keys   yes \
                 delegate-to-first-layer yes)
         (defsrc a b) \
         (deflayer base c (layer-switch 2)) \
         (deflayer 2 _ _)",
        "d:b t:50 u:b t:50 d:a t:50 u:a t:50",
    );
    assert_eq!("t:100ms\nout:↓C\nt:50ms\nout:↑C", result);
}

#[test]
fn delegate_base_but_base_is_transparent() {
    let result = simulate(
        "(defcfg process-unmapped-keys   yes \
                 delegate-to-first-layer yes)
         (defsrc a b) \
         (deflayer base _ (layer-switch 2)) \
         (deflayer 2 _ _)",
        "d:b t:50 u:b t:50 d:a t:50 u:a t:50",
    );
    assert_eq!("t:100ms\nout:↓A\nt:50ms\nout:↑A", result);
}

#[test]
fn layer_switching() {
    let result = simulate(
        "(defcfg process-unmapped-keys   yes
                 delegate-to-first-layer yes)
         (defsrc a b c d)
         (deflayer base x y z (layer-switch 2))
         (deflayer 2 e f _ (layer-switch 3))
         (deflayer 3 g _ _ (layer-switch 4))
         (deflayer 4 _ _ _ XX)
        ",
        "d:c t:20 u:c t:20 d:d t:20 u:d t:20
         d:b t:20 u:b t:20
         d:c t:20 u:c t:20
         d:d t:20 u:d t:20
         d:a t:20 u:a t:20
         d:b t:20 u:b t:20
         d:d t:20 u:d t:20
         d:a t:20 u:a t:20",
    );
    assert_eq!(
        "out:↓Z\nt:20ms\nout:↑Z\nt:60ms\nout:↓F\nt:20ms\nout:↑F\nt:20ms\nout:↓Z\nt:20ms\nout:↑Z\nt:60ms\nout:↓G\nt:20ms\nout:↑G\nt:20ms\nout:↓Y\nt:20ms\nout:↑Y\nt:60ms\nout:↓X\nt:20ms\nout:↑X",
        result
    );
}

#[test]
fn layer_holding() {
    let result = simulate(
        "(defcfg process-unmapped-keys   yes
                 delegate-to-first-layer no)
         (defsrc a b c d e f)
         (deflayer base x y z (layer-while-held 2) XX XX)
         (deflayer 2 e f _ XX (layer-while-held 3) XX)
         (deflayer 3 g _ _ XX XX (layer-while-held 4))
         (deflayer 4 _ _ _ XX XX XX)
        ",
        "d:c t:20 u:c t:20
         d:d t:20
         d:a t:20 u:a t:20
         d:b t:20 u:b t:20
         d:c t:20 u:c t:20
         d:e t:20
         d:a t:20 u:a t:20
         d:b t:20 u:b t:20
         d:c t:20 u:c t:20
         d:f t:20
         d:a t:20 u:a t:20
         d:b t:20 u:b t:20
         d:c t:20 u:c t:20",
    );
    assert_eq!(
        "out:↓Z\nt:20ms\nout:↑Z\nt:40ms\nout:↓E\nt:20ms\nout:↑E\nt:20ms\nout:↓F\nt:20ms\nout:↑F\nt:20ms\nout:↓Z\nt:20ms\nout:↑Z\nt:40ms\nout:↓G\nt:20ms\nout:↑G\nt:20ms\nout:↓F\nt:20ms\nout:↑F\nt:20ms\nout:↓Z\nt:20ms\nout:↑Z\nt:40ms\nout:↓G\nt:20ms\nout:↑G\nt:20ms\nout:↓F\nt:20ms\nout:↑F\nt:20ms\nout:↓Z\nt:20ms\nout:↑Z",
        result
    );
}

// =============================================================================
// Layer Switch Simulator Input Tests
// =============================================================================
// These tests verify the simulator's ability to directly switch layers
// using the ls:name syntax in simulation input strings.

/// Test ls:layer_name to switch to a named layer
#[test]
fn ls_sim_switch_to_layer() {
    const CFG: &str = r"
        (defsrc a b)
        (deflayer base a b)
        (deflayer other 1 2)
    ";
    // Switch to 'other' layer, then press 'a' which should output '1'
    let result = simulate(CFG, "ls:other t:10 d:a t:10 u:a t:10").to_ascii();
    assert_eq!("t:10ms dn:Kb1 t:10ms up:Kb1", result);
}

/// Test ls: can switch between multiple layers
#[test]
fn ls_sim_switch_multiple_layers() {
    const CFG: &str = r"
        (defsrc a b)
        (deflayer base a b)
        (deflayer num 1 2)
        (deflayer nav left right)
    ";
    // Switch to num, press a (outputs 1), switch to nav, press a (outputs left)
    let result = simulate(
        CFG,
        "ls:num d:a t:10 u:a t:10 ls:nav d:a t:10 u:a t:10",
    )
    .to_ascii();
    assert_eq!("dn:Kb1 t:10ms up:Kb1 t:10ms dn:Left t:10ms up:Left", result);
}

/// Test ls: can switch back to base layer
#[test]
fn ls_sim_switch_back_to_base() {
    const CFG: &str = r"
        (defsrc a)
        (deflayer base a)
        (deflayer other 1)
    ";
    // Switch to other (a->1), switch back to base (a->a)
    let result = simulate(
        CFG,
        "ls:other d:a t:10 u:a t:10 ls:base d:a t:10 u:a t:10",
    )
    .to_ascii();
    assert_eq!("dn:Kb1 t:10ms up:Kb1 t:10ms dn:A t:10ms up:A", result);
}

/// Test layer-switch: as alternative syntax
#[test]
fn ls_sim_layer_switch_prefix() {
    const CFG: &str = r"
        (defsrc a)
        (deflayer base a)
        (deflayer other 1)
    ";
    let result = simulate(CFG, "layer-switch:other d:a t:10 u:a t:10").to_ascii();
    assert_eq!("dn:Kb1 t:10ms up:Kb1", result);
}

/// Test ls: with delegate-to-first-layer - transparent keys should delegate to base
#[test]
fn ls_sim_with_delegate_to_first_layer() {
    const CFG: &str = r"
        (defcfg process-unmapped-keys yes
                delegate-to-first-layer yes)
        (defsrc a b)
        (deflayer base x y)
        (deflayer nav left _)
    ";
    // Switch to nav: 'a' -> left, 'b' -> transparent -> delegates to base -> y
    let result = simulate(
        CFG,
        "ls:nav d:a t:10 u:a t:10 d:b t:10 u:b t:10",
    )
    .to_ascii();
    assert_eq!("dn:Left t:10ms up:Left t:10ms dn:Y t:10ms up:Y", result);
}

/// Test ls: with transparent keys but NO delegate-to-first-layer
#[test]
fn ls_sim_transparent_no_delegate() {
    const CFG: &str = r"
        (defcfg process-unmapped-keys yes
                delegate-to-first-layer no)
        (defsrc a b)
        (deflayer base x y)
        (deflayer nav left _)
    ";
    // Switch to nav: 'a' -> left, 'b' -> transparent -> passthrough (outputs B)
    let result = simulate(
        CFG,
        "ls:nav d:a t:10 u:a t:10 d:b t:10 u:b t:10",
    )
    .to_ascii();
    assert_eq!("dn:Left t:10ms up:Left t:10ms dn:B t:10ms up:B", result);
}

// =============================================================================
// End Layer Switch Simulator Input Tests
// =============================================================================
