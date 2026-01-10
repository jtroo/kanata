use super::*;

#[test]
fn hold_for_duration() {
    const CFG: &str = r"
     (defsrc a b c)
     (defvirtualkeys lmet lmet)
     (defalias hm (hold-for-duration 50 lmet))
     (deflayer base
        (multi @hm (macro-repeat 40 @hm))
        (multi 1 @hm)
        (release-key lmet)
     )
    ";
    let result = simulate(CFG, "d:a t:200 u:a t:60").to_ascii();
    assert_eq!("t:1ms dn:LGui t:258ms up:LGui", result);
    let result = simulate(CFG, "d:a u:a t:25 d:c u:c t:25").to_ascii();
    assert_eq!("t:2ms dn:LGui t:23ms up:LGui", result);
    let result = simulate(CFG, "d:a u:a t:25 d:b u:b t:25 d:b u:b t:60").to_ascii();
    assert_eq!(
        "t:2ms dn:LGui t:23ms dn:Kb1 t:1ms up:Kb1 t:24ms dn:Kb1 t:1ms up:Kb1 t:49ms up:LGui",
        result
    );
}

// =============================================================================
// Virtual Key Simulator Input Tests
// =============================================================================
// These tests verify the simulator's ability to directly activate virtual keys
// using the vk:name[:action] syntax in simulation input strings.

/// Test vk:name with default press action
#[test]
fn vk_sim_default_press() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test (multi lctl lalt))
        (deflayer base a)
    ";
    // vk:name without action should default to press
    // Virtual key actions happen immediately (no preceding tick)
    let result = simulate(CFG, "vk:vk_test t:10").to_ascii();
    assert_eq!("dn:LCtrl dn:LAlt", result);
}

/// Test vk:name:press explicit action
#[test]
fn vk_sim_explicit_press() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    // Virtual key actions happen immediately (no preceding tick)
    let result = simulate(CFG, "vk:vk_test:press t:10").to_ascii();
    assert_eq!("dn:LGui", result);
}

/// Test vk:name:p shorthand for press
#[test]
fn vk_sim_press_shorthand() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    let result = simulate(CFG, "vk:vk_test:p t:10").to_ascii();
    assert_eq!("dn:LGui", result);
}

/// Test vk:name:release action
#[test]
fn vk_sim_release() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    // Press first, then release after tick
    let result = simulate(CFG, "vk:vk_test:press t:10 vk:vk_test:release t:10").to_ascii();
    assert_eq!("dn:LGui t:10ms up:LGui", result);
}

/// Test vk:name:tap action (press + release)
#[test]
fn vk_sim_tap() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    // For tap, press happens immediately, then 1ms tick, then release
    let result = simulate(CFG, "vk:vk_test:tap t:10").to_ascii();
    assert_eq!("dn:LGui t:1ms up:LGui", result);
}

/// Test vk:name:t shorthand for tap
#[test]
fn vk_sim_tap_shorthand() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    let result = simulate(CFG, "vk:vk_test:t t:10").to_ascii();
    assert_eq!("dn:LGui t:1ms up:LGui", result);
}

/// Test vk:name:toggle action
#[test]
fn vk_sim_toggle() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    // First toggle: press (key not active -> activate)
    // Second toggle: release (key active -> deactivate)
    let result = simulate(CFG, "vk:vk_test:toggle t:10 vk:vk_test:toggle t:10").to_ascii();
    assert_eq!("dn:LGui t:10ms up:LGui", result);
}

/// Test vk:name:g shorthand for toggle
#[test]
fn vk_sim_toggle_shorthand() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    let result = simulate(CFG, "vk:vk_test:g t:10 vk:vk_test:g t:10").to_ascii();
    assert_eq!("dn:LGui t:10ms up:LGui", result);
}

/// Test fakekey: prefix (alias for vk:)
#[test]
fn vk_sim_fakekey_prefix() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    let result = simulate(CFG, "fakekey:vk_test:tap t:10").to_ascii();
    assert_eq!("dn:LGui t:1ms up:LGui", result);
}

/// Test virtualkey: prefix (alias for vk:)
#[test]
fn vk_sim_virtualkey_prefix() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    let result = simulate(CFG, "virtualkey:vk_test:tap t:10").to_ascii();
    assert_eq!("dn:LGui t:1ms up:LGui", result);
}

/// Test 🎭 emoji prefix
#[test]
fn vk_sim_emoji_prefix() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys vk_test lmet)
        (deflayer base a)
    ";
    let result = simulate(CFG, "🎭:vk_test:tap t:10").to_ascii();
    assert_eq!("dn:LGui t:1ms up:LGui", result);
}

/// Test virtual key with layer switching
#[test]
fn vk_sim_layer_switch() {
    const CFG: &str = r"
        (defsrc a b)
        (defvirtualkeys vk_layer (layer-switch other))
        (deflayer base a b)
        (deflayer other 1 2)
    ";
    // Activate the layer switch virtual key, then press 'a' which should output '1'
    let result = simulate(CFG, "vk:vk_layer t:10 d:a u:a t:10").to_ascii();
    assert_eq!("t:10ms dn:Kb1 t:1ms up:Kb1", result);
}

/// Test multiple virtual keys in sequence
#[test]
fn vk_sim_multiple_vkeys() {
    const CFG: &str = r"
        (defsrc a)
        (defvirtualkeys
            vk_ctrl lctl
            vk_alt lalt
        )
        (deflayer base a)
    ";
    let result = simulate(
        CFG,
        "vk:vk_ctrl:press vk:vk_alt:press t:10 vk:vk_alt:release vk:vk_ctrl:release t:10",
    )
    .to_ascii();
    assert_eq!("dn:LCtrl dn:LAlt t:10ms up:LAlt up:LCtrl", result);
}

// =============================================================================
// End Virtual Key Simulator Input Tests
// =============================================================================

/// Ignored because PRESSED_KEYS is a global static,
/// so shares state with other tests and will fail at random.
/// Should be run on its own until PRESSED_KEYS can be refactored
/// to avoid being a global.
///
/// The "must_be_single_threaded" function naming is referenced
/// in test runners, e.g. justfile and workflows.
#[ignore]
#[test]
fn on_idle_must_be_single_threaded() {
    const CFG: &str = r"
     (defvirtualkeys lmet lmet)
     (defalias i1 (on-idle 20 tap-vkey lmet)
               i2 (on-physical-idle 20 tap-vkey lmet))
     (defsrc a b c)
     (deflayer base
        (caps-word 100) @i1 @i2
     )
    ";
    let result = simulate(
        CFG,
        "d:c t:10 u:c t:5 d:a t:50 u:a t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10",
    )
    .to_ascii();
    assert_eq!("t:86ms dn:LGui t:1ms up:LGui", result);
    let result = simulate(
        CFG,
        "d:b t:10 u:b t:5 d:a t:50 u:a t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10 t:10",
    )
    .to_ascii();
    assert_eq!("t:137ms dn:LGui t:1ms up:LGui", result);
}
