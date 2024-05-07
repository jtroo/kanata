use super::*;

#[test]
fn special_nop_keys() {
    let result = simulate(
        r#"
         (defcfg)
         (defsrc a b)
         (deflayer base (unicode "(") (unicode ")"))
        "#,
        "d:a d:b t:10",
    )
    .no_time();
    assert_eq!("outU:( outU:)", result);
}
