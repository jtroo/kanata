use super::*;

#[test]
fn unicode() {
    let result = simulate(
        r##"
         (defcfg)
         (defsrc 6 7 8 9 0 f1)
         (deflayer base
             (unicode r#"("#)
             (unicode r#")"#)
             (unicode r#"""#)
             (unicode "(")
             (unicode ")")
             (tap-dance 200 (f1(unicode 😀)f2(unicode 🙂)))
         )
        "##,
        "d:6 d:7 d:8 d:9 d:0 t:100",
    )
    .no_time();
    assert_eq!(r#"outU:( outU:) outU:" outU:( outU:)"#, result);
}
