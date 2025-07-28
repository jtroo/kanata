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
    assert_eq!("t:136ms dn:LGui t:1ms up:LGui", result);
}
