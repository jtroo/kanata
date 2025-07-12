use super::*;

#[test]
fn nested_template() {
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
