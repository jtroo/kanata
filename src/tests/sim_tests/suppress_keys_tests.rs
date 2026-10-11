use super::*;

#[test]
fn suppress_keys() {
    let result = simulate(
        "(defsrc)
         (deflayermap base d (suppress-keys a b c))
        ",
        "d:a d:b t:5 d:d t:5 u:d t:10",
    )
    .to_ascii();
    assert_eq!("dn:A t:1ms dn:B t:4ms up:A up:B t:5ms dn:A dn:B", result);
}
