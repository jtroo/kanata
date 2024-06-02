use super::*;

#[test]
fn switch_layer() {
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
