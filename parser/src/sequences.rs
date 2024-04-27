use kanata_keyberon::key_code::KeyCode;

pub const MASK_KEYCODES: u16 = 0x03FF;
pub const MASK_MODDED: u16 = 0xFC00;
pub const KEY_OVERLAP: KeyCode = KeyCode::ErrorRollOver;
pub const KEY_OVERLAP_MARKER: u16 = 0x0400;

pub fn mod_mask_for_keycode(kc: KeyCode) -> u16 {
    use KeyCode::*;
    match kc {
        LShift | RShift => 0x8000,
        LCtrl | RCtrl => 0x4000,
        LAlt => 0x2000,
        RAlt => 0x1000,
        LGui | RGui => 0x0800,
        // This is not real... this is a marker to help signify that key presses should be
        // overlapping. The way this will look in the chord sequence is as such:
        //
        //   [ (0x0400 | X), (0x0400 | Y), (0x0400) ]
        ErrorRollOver => KEY_OVERLAP_MARKER,
        _ => 0,
    }
}

#[test]
fn keys_fit_within_mask() {
    use crate::keys::OsCode;
    assert!(MASK_KEYCODES >= u16::from(OsCode::KEY_MAX));
}
