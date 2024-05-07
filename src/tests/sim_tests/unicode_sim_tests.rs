use super::*;

#[test]
fn special_nop_keys() {
    let result = simulate(
        r##"
         (defcfg)
         (defsrc 6 7 8 9 0)
         (deflayer base
             (unicode r#"("#)
             (unicode r#")"#)
             (unicode r#"""#)
             (unicode "(")
             (unicode ")")
         )
        "##,
        "d:6 d:7 d:8 d:9 d:0 t:100",
    )
    .no_time();
    assert_eq!(r#"outU:( outU:) outU:" outU:( outU:)"#, result);
}
