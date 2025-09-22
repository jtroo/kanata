use super::*;

#[test]
fn use_defsrc_deflayer() {
    let result = simulate(
        r##"
         (defcfg)
         (defsrc a b c d)
         (deflayer base
            1 2 3 (layer-while-held other)
         )
         (deflayer other
            4 5 (layer-while-held src) XX
         )
         (deflayer src
            use-defsrc use-defsrc XX XX
         )
        "##,
        "d:d d:c d:b d:a t:100",
    )
    .to_ascii();
    assert_eq!("t:2ms dn:B t:1ms dn:A", result);
}

#[test]
fn use_defsrc_deflayermap() {
    const CFG: &str = "
         (defcfg process-unmapped-keys yes)
         (defsrc a b c d)
         (deflayer base
            1
            (layer-while-held othermap1)
            (layer-while-held othermap2)
            (layer-while-held othermap3)
         )
         (deflayermap (othermap1)
            a 5
            ___ use-defsrc
         )
         (deflayermap (othermap2)
            a 6
            __ use-defsrc
            _ x
         )
         (deflayermap (othermap3)
            a 7
            _ use-defsrc
            __ x
         )
        ";
    let result = simulate(CFG, "d:b d:a d:c d:e t:10").to_ascii();
    assert_eq!("t:1ms dn:Kb5 t:1ms dn:C t:1ms dn:E", result);
    let result = simulate(CFG, "d:c d:a d:c d:e t:10").to_ascii();
    assert_eq!("t:1ms dn:Kb6 t:1ms dn:X t:1ms dn:E", result);
    let result = simulate(CFG, "d:d d:a d:c d:e t:10").to_ascii();
    assert_eq!("t:1ms dn:Kb7 t:1ms dn:C t:1ms dn:X", result);
}
