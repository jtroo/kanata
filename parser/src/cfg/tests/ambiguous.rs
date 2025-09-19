use super::*;

#[test]
fn parse_double_dollar_var() {
    let source = r#"
(defsrc)
(deflayer base)
(defvar $$num 100
         $num 99
          num not-a-number-or-key)
(defalias test
         (movemouse-accel-up $$num $$$num $$num $$$num))
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_double_at_alias() {
    let source = r#"
(defsrc)
(deflayer base)
          ;; alias cannot be used in macro, @alias can
(defalias @alias 0
           alias (tap-hold 9 9 a b)
           test (macro @@alias))
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}
