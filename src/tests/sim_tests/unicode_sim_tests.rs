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

#[test]
#[cfg(target_os = "macos")]
fn macos_unicode_handling() {
    let result = simulate(
        r##"
         (defcfg)
         (defsrc a)
         (deflayer base
             (unicode "🎉")  ;; Test with an emoji that uses multi-unit UTF-16
         )
        "##,
        "d:a t:100",
    )
    .no_time();
    assert_eq!("outU:🎉", result);
}

#[test]
fn unicode_pulus() {
    let result = simulate(
        "
(defsrc a b)
(deflayer _
 (unicode 🚆)
 (unicode U+1F686)
)
        ",
        "d:a t:10 d:b t:10",
    )
    .no_time();
    assert_eq!("outU:🚆 outU:🚆", result);
}
