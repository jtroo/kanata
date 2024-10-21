use super::*;

#[cfg(feature = "zippychord")]
mod zippychord;
#[cfg(feature = "zippychord")]
pub(crate) use zippychord::*;

// Functions to send keys except those that fall in the ignorable range.
// And also have been repurposed to have additional logic to send mouse events, out of convenience.
//
// POTENTIAL PROBLEM - G-keys:
// Some keys are ignored because they are *probably* unused,
// or otherwise are probably in an unergonomic, far away key position,
// so if you're using kanata, you can now stop using those keys and
// do something better!
//
// I should probably let people turn this off if they really want to,
// but I don't like how that would require extra code.
// I'll defer to YAGNI and add docs, and let people report problems if
// they want a fix 🐝.
//
// The keys ignored are intentionally the upper numbers of KEY_MACROX.
// The Linux input-event-codes.h file mentions G1-G18 and S1-S30
// as keys that might use these codes.
//
// Logitech still makes devices with G-keys
// but the S-keys are apparently from the
// "Microsoft SideWinder X6 Keyboard"
// which appears to no longer be in production.
//
// Thus based on my reading, 18 is the highest macro key
// that can be assumed to be used by devices still in production.
pub(super) const KEY_IGNORE_MIN: u16 = 0x2a4; // KEY_MACRO21
pub(super) const KEY_IGNORE_MAX: u16 = 0x2ad; // KEY_MACRO30
pub(super) fn write_key(kb: &mut KbdOut, osc: OsCode, val: KeyValue) -> Result<(), std::io::Error> {
    match u16::from(osc) {
        KEY_IGNORE_MIN..=KEY_IGNORE_MAX => Ok(()),
        _ => kb.write_key(osc, val),
    }
}
pub(super) fn press_key(kb: &mut KbdOut, osc: OsCode) -> Result<(), std::io::Error> {
    use OsCode::*;
    match u16::from(osc) {
        KEY_IGNORE_MIN..=KEY_IGNORE_MAX => Ok(()),
        _ => match osc {
            BTN_LEFT | BTN_RIGHT | BTN_MIDDLE | BTN_SIDE | BTN_EXTRA => {
                let btn = osc_to_btn(osc);
                kb.click_btn(btn)
            }
            MouseWheelUp | MouseWheelDown | MouseWheelLeft | MouseWheelRight => {
                let direction = osc_to_wheel_direction(osc);
                kb.scroll(direction, HI_RES_SCROLL_UNITS_IN_LO_RES)
            }
            _ => post_filter_press(kb, osc),
        },
    }
}
pub(super) fn release_key(kb: &mut KbdOut, osc: OsCode) -> Result<(), std::io::Error> {
    use OsCode::*;
    match u16::from(osc) {
        KEY_IGNORE_MIN..=KEY_IGNORE_MAX => Ok(()),
        _ => match osc {
            BTN_LEFT | BTN_RIGHT | BTN_MIDDLE | BTN_SIDE | BTN_EXTRA => {
                let btn = osc_to_btn(osc);
                kb.release_btn(btn)
            }
            MouseWheelUp | MouseWheelDown | MouseWheelLeft | MouseWheelRight => {
                // no-op: these are handled as scroll events in the press but scroll has no notion
                // of release.
                Ok(())
            }
            _ => post_filter_release(kb, osc),
        },
    }
}
fn osc_to_btn(osc: OsCode) -> Btn {
    use Btn::*;
    use OsCode::*;
    match osc {
        BTN_LEFT => Left,
        BTN_RIGHT => Right,
        BTN_MIDDLE => Mid,
        BTN_EXTRA => Forward,
        BTN_SIDE => Backward,
        _ => unreachable!("called osc_to_btn with bad value {osc}"),
    }
}
fn osc_to_wheel_direction(osc: OsCode) -> MWheelDirection {
    use MWheelDirection::*;
    use OsCode::*;
    match osc {
        MouseWheelUp => Up,
        MouseWheelDown => Down,
        MouseWheelLeft => Left,
        MouseWheelRight => Right,
        _ => unreachable!("called osc_to_wheel_direction with bad value {osc}"),
    }
}

fn post_filter_press(kb: &mut KbdOut, osc: OsCode) -> Result<(), std::io::Error> {
    #[cfg(not(feature = "zippychord"))]
    {
        kb.press_key(osc)
    }
    #[cfg(feature = "zippychord")]
    {
        zch().zch_press_key(kb, osc)
    }
}

fn post_filter_release(kb: &mut KbdOut, osc: OsCode) -> Result<(), std::io::Error> {
    #[cfg(not(feature = "zippychord"))]
    {
        kb.release_key(osc)
    }
    #[cfg(feature = "zippychord")]
    {
        zch().zch_release_key(kb, osc)
    }
}

pub(super) fn zippy_is_idle() -> bool {
    #[cfg(not(feature = "zippychord"))]
    {
        true
    }
    #[cfg(feature = "zippychord")]
    {
        zch().zch_is_idle()
    }
}

pub(super) fn zippy_tick(_caps_word_is_active: bool) {
    #[cfg(feature = "zippychord")]
    {
        zch().zch_tick(_caps_word_is_active)
    }
}
