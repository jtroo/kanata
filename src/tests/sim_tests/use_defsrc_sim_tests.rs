use super::*;

#[test]
fn use_defsrc() {
    let result = simulate(
        r##"
         (defcfg)
         (defsrc a b c)
         (deflayer base
            1 2 (layer-while-held other)
         )
         (deflayer other
            3 (layer-while-held src) XX
         )
         (deflayer src
            use-defsrc XX XX
         )
        "##,
        "d:c d:b d:a t:100",
    )
    .to_ascii();
    assert_eq!("t:2ms dn:A", result);
}
