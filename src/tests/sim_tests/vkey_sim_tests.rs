use super::*;

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

#[test]
fn hold_for_duration() {
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
