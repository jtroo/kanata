use super::*;

#[test]
fn tap_dance_eager_with_mlft() {
    let result = simulate(
        "
        (defsrc)
        (deflayermap (baselayer)
            a (tap-dance-eager 200 (mlft mrgt mmid))
        )
        ",
        "
        d:a t:1 u:a t:1
        d:a t:1 u:a t:1
        d:a t:1 u:a t:1
        ",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:↓Left t:1ms out🖰:↑Left t:1ms out🖰:↓Right t:1ms out🖰:↑Right t:1ms out🖰:↓Mid t:1ms out🖰:↑Mid",
        result
    );
}
