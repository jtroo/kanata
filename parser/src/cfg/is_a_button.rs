
pub(crate) fn is_a_button(osc: u16) -> bool {
    #[cfg(not(target_os = "windows"))]
    match osc {
        256..=337 | 544..=547 | 704..=748 => true,
        _ => false,
    }
    #[cfg(target_os = "windows")]
    match osc {
        1..=6 | 256..=337 | 544..=547 | 704..=748 => true,
        _ => false,
    }
}

#[test]
fn mouse_inputs_most_care_about_are_considered_buttons() {
    use crate::keys::{OsCode, OsCode::*};
    const MOUSE_INPUTS: &[OsCode] = &[
        MouseWheelUp,
        MouseWheelDown,
        MouseWheelLeft,
        MouseWheelRight,
        BTN_LEFT,
        BTN_RIGHT,
        BTN_MIDDLE,
        BTN_SIDE,
        BTN_EXTRA,
        BTN_FORWARD,
        BTN_BACK,
    ];
    for input in MOUSE_INPUTS.iter().copied() {
        dbg!(input);
        assert!(is_a_button(dbg!(input.into())));
    }
}
