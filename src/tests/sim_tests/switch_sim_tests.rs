use super::*;

#[test]
fn sim_switch_layer() {
    let result = simulate(
        "
         (defcfg)
         (defsrc a b)
         (defalias b (switch
            ((layer base)) x break
            ((layer other)) y break))
         (deflayer base (layer-while-held other) @b)
         (deflayer other XX @b)
        ",
        "d:b u:b t:10 d:a d:b u:b u:a t:10",
    )
    .no_time();
    assert_eq!("out:↓X out:↑X out:↓Y out:↑Y", result);
}

#[test]
fn sim_switch_base_layer() {
    let result = simulate(
        "
         (defcfg)
         (defsrc a b c)
         (defalias b (switch
            ((base-layer base)) x break
            ((base-layer other)) y break))
         (deflayer base (layer-switch other) @b c)
         (deflayer other XX @b (layer-while-held base))
        ",
        "d:b u:b t:10 d:a d:b u:b u:a t:10 d:c t:10 d:b t:10 u:c u:b t:10",
    )
    .no_time();
    assert_eq!("out:↓X out:↑X out:↓Y out:↑Y out:↓Y out:↑Y", result);
}

#[test]
fn sim_switch_noop() {
    let result = simulate(
        "
         (defsrc)
         (deflayermap (-) a XX b (switch
          ((input real a)) c break
          () d break
         ))
        ",
        "d:a d:b t:10 u:a u:b t:10 d:b u:b t:10",
    )
    .no_time();
    assert_eq!("out:↓C out:↑C out:↓D out:↑D", result);
}
