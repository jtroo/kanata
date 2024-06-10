use super::*;

#[test]
fn override_with_unmod() {
    let result = simulate(
        "
(defoverrides
 (a) (b)
 (b) (a)
)

(defalias
 b (unshift b)
 a (unshift a)
)
(defsrc a b)
(deflayer base @a @b)
        ",
        "d:lsft t:50 d:a t:50 u:a t:50 d:b t:50 u:b t:50",
    )
    .to_ascii()
    .no_time();
    assert_eq!(
        "dn:LShift up:LShift dn:B up:B dn:LShift up:LShift dn:A up:A dn:LShift",
        result
    );
}
