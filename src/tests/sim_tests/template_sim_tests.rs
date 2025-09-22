use super::*;

#[test]
fn nested_template() {
    let result = simulate(
        "
        (deftemplate one (v1)
         a b c $v1
        )
        (deftemplate two (v2)
         (t! one $v2)
         e f g
        )
        (defsrc        (t! two d))
        (deflayer base (t! two x))
        ",
        "d:a t:10 u:a t:10 d:d t:10 u:d t:10 d:g t:10 u:g t:10",
    )
    .no_time();
    assert_eq!("out:↓A out:↑A out:↓X out:↑X out:↓G out:↑G", result);
}
