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

#[test]
fn sim_switch_trans_not_top_layer() {
    let result = simulate(
        "
        (defalias init (multi (layer-while-held l1) (layer-while-held l2) (layer-while-held l3) (layer-while-held l4)))
        (defsrc a b)
        (deflayer l0 c @init)
        (deflayer l1 b @init)
        (deflayer l2 (switch () _ break) @init)
        (deflayer l3 _ @init)
        (deflayer l4 _ @init)
        ",
        "d:b t:20 d:a t:10 u:a t:100 d:a t:10 u:a t:100",
    )
    .to_ascii();
    assert_eq!("t:21ms dn:B t:9ms up:B t:101ms dn:B t:9ms up:B", result);
}
