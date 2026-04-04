use super::*;

#[test]
fn multi_mouse_button_does_multi_click_release_single_hold() {
    let result = simulate(
        "(defsrc) (deflayermap (base) a (multi mmid mmid mrgt mlft))",
        "d:a t:50 u:a t:50",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:↓Mid out🖰:↑Mid t:1ms out🖰:↓Mid out🖰:↑Mid t:1ms out🖰:↓Right out🖰:↑Right t:1ms out🖰:↓Left t:50ms out🖰:↑Left",
        result
    );
}
