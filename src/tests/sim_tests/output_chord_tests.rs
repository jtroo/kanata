use super::*;

#[test]
fn output_chord_samekey_has_release() {
    let result = simulate(
        "
        (defsrc a b)
        (deflayer _ S-= =)
        ",
        "d:a t:10 d:b t:10",
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift dn:Equal t:10ms up:LShift up:Equal t:1ms dn:Equal",
        result
    );
}
