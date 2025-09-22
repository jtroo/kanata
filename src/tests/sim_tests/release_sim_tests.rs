use super::*;

#[test]
fn release_standard() {
    let result = simulate(
        "
         (defsrc a)
         (deflayer base (multi lalt a))
        ",
        "
         d:a t:10 u:a t:10
        ",
    )
    .to_ascii();
    assert_eq!("dn:LAlt dn:A t:10ms up:LAlt up:A", result);
}

#[test]
fn release_reversed() {
    let result = simulate(
        "
         (defsrc a)
         (deflayer base (multi lalt a reverse-release-order))
        ",
        "
         d:a t:10 u:a t:10
        ",
    )
    .to_ascii();
    assert_eq!("dn:LAlt dn:A t:10ms up:A up:LAlt", result);
}
