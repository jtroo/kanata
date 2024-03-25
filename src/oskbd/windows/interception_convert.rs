//! `Interception::Stroke` conversion functions
//!
//! The keyboard scancode values come from this website:
//! https://handmade.network/forums/articles/t/2823-keyboard_inputs_-_scancodes%252C_raw_input%252C_text_input%252C_key_names
//!
//! Which states that it got these values from:
//! - http://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/scancode.doc (March 16, 2000).
//! - http://www.computer-engineering.org/ps2keyboard/scancodes1.html
//! - using MapVirtualKeyEx( VK_*, MAPVK_VK_TO_VSC_EX, 0 ) with the english us keyboard layout
//! - reading win32 WM_INPUT keyboard messages.

/*
enum Scancode {
    sc_escape = 0x01,
    sc_1 = 0x02,
    sc_2 = 0x03,
    sc_3 = 0x04,
    sc_4 = 0x05,
    sc_5 = 0x06,
    sc_6 = 0x07,
    sc_7 = 0x08,
    sc_8 = 0x09,
    sc_9 = 0x0A,
    sc_0 = 0x0B,
    sc_minus = 0x0C,
    sc_equals = 0x0D,
    sc_backspace = 0x0E,
    sc_tab = 0x0F,
    sc_q = 0x10,
    sc_w = 0x11,
    sc_e = 0x12,
    sc_r = 0x13,
    sc_t = 0x14,
    sc_y = 0x15,
    sc_u = 0x16,
    sc_i = 0x17,
    sc_o = 0x18,
    sc_p = 0x19,
    sc_bracketLeft = 0x1A,
    sc_bracketRight = 0x1B,
    sc_enter = 0x1C,
    sc_controlLeft = 0x1D,
    sc_a = 0x1E,
    sc_s =0x1F,
    sc_d = 0x20,
    sc_f = 0x21,
    sc_g = 0x22,
    sc_h = 0x23,
    sc_j = 0x24,
    sc_k = 0x25,
    sc_l = 0x26,
    sc_semicolon = 0x27,
    sc_apostrophe = 0x28,
    sc_grave = 0x29,
    sc_shiftLeft = 0x2A,
    sc_backslash = 0x2B,
    sc_z = 0x2C,
    sc_x = 0x2D,
    sc_c = 0x2E,
    sc_v = 0x2F,
    sc_b = 0x30,
    sc_n = 0x31,
    sc_m = 0x32,
    sc_comma = 0x33,
    sc_preiod = 0x34,
    sc_slash = 0x35,
    sc_shiftRight = 0x36,
    sc_numpad_multiply = 0x37,
    sc_altLeft = 0x38,
    sc_space = 0x39,
    sc_capsLock = 0x3A,
    sc_f1 = 0x3B,
    sc_f2 = 0x3C,
    sc_f3 = 0x3D,
    sc_f4 = 0x3E,
    sc_f5 = 0x3F,
    sc_f6 = 0x40,
    sc_f7 = 0x41,
    sc_f8 = 0x42,
    sc_f9 = 0x43,
    sc_f10 = 0x44,
    sc_numLock = 0x45,
    sc_scrollLock = 0x46,
    sc_numpad_7 = 0x47,
    sc_numpad_8 = 0x48,
    sc_numpad_9 = 0x49,
    sc_numpad_minus = 0x4A,
    sc_numpad_4 = 0x4B,
    sc_numpad_5 = 0x4C,
    sc_numpad_6 = 0x4D,
    sc_numpad_plus = 0x4E,
    sc_numpad_1 = 0x4F,
    sc_numpad_2 = 0x50,
    sc_numpad_3 = 0x51,
    sc_numpad_0 = 0x52,
    sc_numpad_period = 0x53,
    sc_alt_printScreen = 0x54, /* Alt + print screen. MapVirtualKeyEx( VK_SNAPSHOT, MAPVK_VK_TO_VSC_EX, 0 ) returns scancode 0x54. */
    sc_bracketAngle = 0x56, /* Key between the left shift and Z. */
    sc_f11 = 0x57,
    sc_f12 = 0x58,
    sc_oem_1 = 0x5a, /* VK_OEM_WSCTRL */
    sc_oem_2 = 0x5b, /* VK_OEM_FINISH */
    sc_oem_3 = 0x5c, /* VK_OEM_JUMP */
    sc_eraseEOF = 0x5d,
    sc_oem_4 = 0x5e, /* VK_OEM_BACKTAB */
    sc_oem_5 = 0x5f, /* VK_OEM_AUTO */
    sc_zoom = 0x62,
    sc_help = 0x63,
    sc_f13 = 0x64,
    sc_f14 = 0x65,
    sc_f15 = 0x66,
    sc_f16 = 0x67,
    sc_f17 = 0x68,
    sc_f18 = 0x69,
    sc_f19 = 0x6a,
    sc_f20 = 0x6b,
    sc_f21 = 0x6c,
    sc_f22 = 0x6d,
    sc_f23 = 0x6e,
    sc_oem_6 = 0x6f, /* VK_OEM_PA3 */
    sc_katakana = 0x70,
    sc_oem_7 = 0x71, /* VK_OEM_RESET */
    sc_f24 = 0x76,
    sc_sbcschar = 0x77,
    sc_convert = 0x79,
    sc_nonconvert = 0x7B, /* VK_OEM_PA1 */
    sc_media_previous = 0xE010,
    sc_media_next = 0xE019,
    sc_numpad_enter = 0xE01C,
    sc_controlRight = 0xE01D,
    sc_volume_mute = 0xE020,
    sc_launch_app2 = 0xE021,
    sc_media_play = 0xE022,
    sc_media_stop = 0xE024,
    sc_volume_down = 0xE02E,
    sc_volume_up = 0xE030,
    sc_browser_home = 0xE032,
    sc_numpad_divide = 0xE035,
    sc_printScreen = 0xE037,
    /*
    sc_printScreen:
    - make: 0xE02A 0xE037
    - break: 0xE0B7 0xE0AA
    - MapVirtualKeyEx( VK_SNAPSHOT, MAPVK_VK_TO_VSC_EX, 0 ) returns scancode 0x54;
    - There is no VK_KEYDOWN with VK_SNAPSHOT.
    */
    sc_altRight = 0xE038,
    sc_cancel = 0xE046, /* CTRL + Pause */
    sc_home = 0xE047,
    sc_arrowUp = 0xE048,
    sc_pageUp = 0xE049,
    sc_arrowLeft = 0xE04B,
    sc_arrowRight = 0xE04D,
    sc_end = 0xE04F,
    sc_arrowDown = 0xE050,
    sc_pageDown = 0xE051,
    sc_insert = 0xE052,
    sc_delete = 0xE053,
    sc_metaLeft = 0xE05B,
    sc_metaRight = 0xE05C,
    sc_application = 0xE05D,
    sc_power = 0xE05E,
    sc_sleep = 0xE05F,
    sc_wake = 0xE063,
    sc_browser_search = 0xE065,
    sc_browser_favorites = 0xE066,
    sc_browser_refresh = 0xE067,
    sc_browser_stop = 0xE068,
    sc_browser_forward = 0xE069,
    sc_browser_back = 0xE06A,
    sc_launch_app1 = 0xE06B,
    sc_launch_email = 0xE06C,
    sc_launch_media = 0xE06D,
    sc_pause = 0xE11D45,
    /*
    sc_pause:
    - make: 0xE11D 45 0xE19D C5
    - make in raw input: 0xE11D 0x45
    - break: none
    - No repeat when you hold the key down
    - There are no break so I don't know how the key down/up is expected to work. Raw input sends "keydown" and "keyup" messages, and it appears that the keyup message is sent directly after the keydown message (you can't hold the key down) so depending on when GetMessage or PeekMessage will return messages, you may get both a keydown and keyup message "at the same time". If you use VK messages most of the time you only get keydown messages, but some times you get keyup messages too.
    - when pressed at the same time as one or both control keys, generates a 0xE046 (sc_cancel) and the string for that scancode is "break".
    */
}
*/

use kanata_interception::*;
use kanata_parser::keys::OsCode;

// We need to wrap OsCode to impl TryFrom<..> for it, because it's in external crate.
pub struct OsCodeWrapper(pub OsCode);

impl TryFrom<Stroke> for OsCodeWrapper {
    type Error = ();

    fn try_from(item: Stroke) -> Result<Self, Self::Error> {
        Ok(match item {
            Stroke::Keyboard { code, state, .. } => {
                let code = match (state.contains(KeyState::E0), state.contains(KeyState::E1)) {
                    (false, false) => crate::oskbd::u16_to_osc(code as u16).ok_or(())?,
                    (true, _) => crate::oskbd::u16_to_osc((code as u16) | 0xE000).ok_or(())?,
                    _ => return Err(()),
                };
                OsCodeWrapper(code)
            }
            _ => return Err(()),
        })
    }
}

impl TryFrom<OsCodeWrapper> for Stroke {
    type Error = ();

    fn try_from(item: OsCodeWrapper) -> Result<Self, Self::Error> {
        let (code, state) = match item.0 {
            OsCode::KEY_ESC => (ScanCode::Esc, KeyState::empty()),
            OsCode::KEY_1 => (ScanCode::Num1, KeyState::empty()),
            OsCode::KEY_2 => (ScanCode::Num2, KeyState::empty()),
            OsCode::KEY_3 => (ScanCode::Num3, KeyState::empty()),
            OsCode::KEY_4 => (ScanCode::Num4, KeyState::empty()),
            OsCode::KEY_5 => (ScanCode::Num5, KeyState::empty()),
            OsCode::KEY_6 => (ScanCode::Num6, KeyState::empty()),
            OsCode::KEY_7 => (ScanCode::Num7, KeyState::empty()),
            OsCode::KEY_8 => (ScanCode::Num8, KeyState::empty()),
            OsCode::KEY_9 => (ScanCode::Num9, KeyState::empty()),
            OsCode::KEY_0 => (ScanCode::Num0, KeyState::empty()),
            OsCode::KEY_MINUS => (ScanCode::Minus, KeyState::empty()),
            OsCode::KEY_EQUAL => (ScanCode::Equals, KeyState::empty()),
            OsCode::KEY_BACKSPACE => (ScanCode::Backspace, KeyState::empty()),
            OsCode::KEY_TAB => (ScanCode::Tab, KeyState::empty()),
            OsCode::KEY_Q => (ScanCode::Q, KeyState::empty()),
            OsCode::KEY_W => (ScanCode::W, KeyState::empty()),
            OsCode::KEY_E => (ScanCode::E, KeyState::empty()),
            OsCode::KEY_R => (ScanCode::R, KeyState::empty()),
            OsCode::KEY_T => (ScanCode::T, KeyState::empty()),
            OsCode::KEY_Y => (ScanCode::Y, KeyState::empty()),
            OsCode::KEY_U => (ScanCode::U, KeyState::empty()),
            OsCode::KEY_I => (ScanCode::I, KeyState::empty()),
            OsCode::KEY_O => (ScanCode::O, KeyState::empty()),
            OsCode::KEY_P => (ScanCode::P, KeyState::empty()),
            OsCode::KEY_LEFTBRACE => (ScanCode::LeftBracket, KeyState::empty()),
            OsCode::KEY_RIGHTBRACE => (ScanCode::RightBracket, KeyState::empty()),
            OsCode::KEY_ENTER => (ScanCode::Enter, KeyState::empty()),
            OsCode::KEY_LEFTCTRL => (ScanCode::LeftControl, KeyState::empty()),
            OsCode::KEY_A => (ScanCode::A, KeyState::empty()),
            OsCode::KEY_S => (ScanCode::S, KeyState::empty()),
            OsCode::KEY_D => (ScanCode::D, KeyState::empty()),
            OsCode::KEY_F => (ScanCode::F, KeyState::empty()),
            OsCode::KEY_G => (ScanCode::G, KeyState::empty()),
            OsCode::KEY_H => (ScanCode::H, KeyState::empty()),
            OsCode::KEY_J => (ScanCode::J, KeyState::empty()),
            OsCode::KEY_K => (ScanCode::K, KeyState::empty()),
            OsCode::KEY_L => (ScanCode::L, KeyState::empty()),
            OsCode::KEY_SEMICOLON => (ScanCode::SemiColon, KeyState::empty()),
            OsCode::KEY_APOSTROPHE => (ScanCode::Apostrophe, KeyState::empty()),
            OsCode::KEY_GRAVE => (ScanCode::Grave, KeyState::empty()),
            OsCode::KEY_LEFTSHIFT => (ScanCode::LeftShift, KeyState::empty()),
            OsCode::KEY_BACKSLASH => (ScanCode::BackSlash, KeyState::empty()),
            OsCode::KEY_Z => (ScanCode::Z, KeyState::empty()),
            OsCode::KEY_X => (ScanCode::X, KeyState::empty()),
            OsCode::KEY_C => (ScanCode::C, KeyState::empty()),
            OsCode::KEY_V => (ScanCode::V, KeyState::empty()),
            OsCode::KEY_B => (ScanCode::B, KeyState::empty()),
            OsCode::KEY_N => (ScanCode::N, KeyState::empty()),
            OsCode::KEY_M => (ScanCode::M, KeyState::empty()),
            OsCode::KEY_COMMA => (ScanCode::Comma, KeyState::empty()),
            OsCode::KEY_DOT => (ScanCode::Period, KeyState::empty()),
            OsCode::KEY_SLASH => (ScanCode::Slash, KeyState::empty()),
            OsCode::KEY_RIGHTSHIFT => (ScanCode::RightShift, KeyState::empty()),
            OsCode::KEY_KPASTERISK => (ScanCode::NumpadMultiply, KeyState::empty()),
            OsCode::KEY_LEFTALT => (ScanCode::LeftAlt, KeyState::empty()),
            OsCode::KEY_SPACE => (ScanCode::Space, KeyState::empty()),
            OsCode::KEY_CAPSLOCK => (ScanCode::CapsLock, KeyState::empty()),
            OsCode::KEY_F1 => (ScanCode::F1, KeyState::empty()),
            OsCode::KEY_F2 => (ScanCode::F2, KeyState::empty()),
            OsCode::KEY_F3 => (ScanCode::F3, KeyState::empty()),
            OsCode::KEY_F4 => (ScanCode::F4, KeyState::empty()),
            OsCode::KEY_F5 => (ScanCode::F5, KeyState::empty()),
            OsCode::KEY_F6 => (ScanCode::F6, KeyState::empty()),
            OsCode::KEY_F7 => (ScanCode::F7, KeyState::empty()),
            OsCode::KEY_F8 => (ScanCode::F8, KeyState::empty()),
            OsCode::KEY_F9 => (ScanCode::F9, KeyState::empty()),
            OsCode::KEY_F10 => (ScanCode::F10, KeyState::empty()),
            OsCode::KEY_NUMLOCK => (ScanCode::NumLock, KeyState::empty()),
            OsCode::KEY_SCROLLLOCK => (ScanCode::ScrollLock, KeyState::empty()),
            OsCode::KEY_KP7 => (ScanCode::Numpad7, KeyState::empty()),
            OsCode::KEY_KP8 => (ScanCode::Numpad8, KeyState::empty()),
            OsCode::KEY_KP9 => (ScanCode::Numpad9, KeyState::empty()),
            OsCode::KEY_KPMINUS => (ScanCode::NumpadMinus, KeyState::empty()),
            OsCode::KEY_KP4 => (ScanCode::Numpad4, KeyState::empty()),
            OsCode::KEY_KP5 => (ScanCode::Numpad5, KeyState::empty()),
            OsCode::KEY_KP6 => (ScanCode::Numpad6, KeyState::empty()),
            OsCode::KEY_KPPLUS => (ScanCode::NumpadPlus, KeyState::empty()),
            OsCode::KEY_KP1 => (ScanCode::Numpad1, KeyState::empty()),
            OsCode::KEY_KP2 => (ScanCode::Numpad2, KeyState::empty()),
            OsCode::KEY_KP3 => (ScanCode::Numpad3, KeyState::empty()),
            OsCode::KEY_KP0 => (ScanCode::Numpad0, KeyState::empty()),
            OsCode::KEY_KPDOT => (ScanCode::NumpadPeriod, KeyState::empty()),
            OsCode::KEY_102ND => (ScanCode::Int1, KeyState::empty()), /* Key between the left shift and Z. */
            OsCode::KEY_F11 => (ScanCode::F11, KeyState::empty()),
            OsCode::KEY_F12 => (ScanCode::F12, KeyState::empty()),
            OsCode::KEY_F13 => (ScanCode::F13, KeyState::empty()),
            OsCode::KEY_F14 => (ScanCode::F14, KeyState::empty()),
            OsCode::KEY_F15 => (ScanCode::F15, KeyState::empty()),
            OsCode::KEY_F16 => (ScanCode::F16, KeyState::empty()),
            OsCode::KEY_F17 => (ScanCode::F17, KeyState::empty()),
            OsCode::KEY_F18 => (ScanCode::F18, KeyState::empty()),
            OsCode::KEY_F19 => (ScanCode::F19, KeyState::empty()),
            OsCode::KEY_F20 => (ScanCode::F20, KeyState::empty()),
            OsCode::KEY_F21 => (ScanCode::F21, KeyState::empty()),
            OsCode::KEY_F22 => (ScanCode::F22, KeyState::empty()),
            OsCode::KEY_F23 => (ScanCode::F23, KeyState::empty()),
            OsCode::KEY_F24 => (ScanCode::F24, KeyState::empty()),
            OsCode::KEY_KATAKANA => (ScanCode::Katakana, KeyState::empty()),
            // Note: the OEM keys below don't seem to correspond to the same VK OEM
            // mappings as the LLHOOK codes.
            // ScanCode::Oem1 = 0x5A, /* VK_OEM_WSCTRL */
            // ScanCode::Oem2 = 0x5B, /* VK_OEM_FINISH */
            // ScanCode::Oem3 = 0x5C, /* VK_OEM_JUMP */
            // ScanCode::Oem4 = 0x5E, /* VK_OEM_BACKTAB */
            // ScanCode::Oem5 = 0x5F, /* VK_OEM_AUTO */
            // ScanCode::Oem6 = 0x6F, /* VK_OEM_PA3 */
            // ScanCode::Oem7 = 0x71, /* VK_OEM_RESET */
            // ScanCode::EraseEOF = 0x5D,
            // ScanCode::Zoom => 0x62,
            // ScanCode::Help => 0x63,
            // ScanCode::AltPrintScreen = 0x55, /* Alt + print screen. */
            // ScanCode::SBCSChar = 0x77,
            // ScanCode::Convert = 0x79,
            // ScanCode::NonConvert = 0x7B,
            OsCode::KEY_PREVIOUSSONG => (ScanCode::Q, KeyState::E0),
            OsCode::KEY_NEXTSONG => (ScanCode::P, KeyState::E0), // 0x19
            OsCode::KEY_KPENTER => (ScanCode::Enter, KeyState::E0), // 0x1C
            OsCode::KEY_RIGHTCTRL => (ScanCode::LeftControl, KeyState::E0), // 0x1D
            OsCode::KEY_MUTE => (ScanCode::D, KeyState::E0),     // 0x20
            OsCode::KEY_PLAYPAUSE => (ScanCode::G, KeyState::E0), // 0x22 // sc_media_play
            OsCode::KEY_VOLUMEDOWN => (ScanCode::C, KeyState::E0), // 0x2E // sc_volume_down
            OsCode::KEY_VOLUMEUP => (ScanCode::B, KeyState::E0), // 0x30   // sc_volume_up
            OsCode::KEY_KPSLASH => (ScanCode::Slash, KeyState::E0), // 0x35 // sc_numpad_divide
            OsCode::KEY_PRINT => (ScanCode::NumpadMultiply, KeyState::E0), // 0x37   // sc_printScreen
            OsCode::KEY_RIGHTALT => (ScanCode::LeftAlt, KeyState::E0),     // 0x38 // sc_altRight
            OsCode::KEY_HOME => (ScanCode::Numpad7, KeyState::E0),         // 0x47     // sc_home
            OsCode::KEY_UP => (ScanCode::Numpad8, KeyState::E0), // 0x48       // sc_arrowUp
            OsCode::KEY_PAGEUP => (ScanCode::Numpad9, KeyState::E0), // 0x49   // sc_pageUp
            OsCode::KEY_LEFT => (ScanCode::Numpad4, KeyState::E0), // 0x4B     // sc_arrowLeft
            OsCode::KEY_RIGHT => (ScanCode::Numpad6, KeyState::E0), // 0x4D    // sc_arrowRight
            OsCode::KEY_END => (ScanCode::Numpad1, KeyState::E0), // 0x4F      // sc_end
            OsCode::KEY_DOWN => (ScanCode::Numpad2, KeyState::E0), // 0x50     // sc_arrowDown
            OsCode::KEY_PAGEDOWN => (ScanCode::Numpad3, KeyState::E0), // 0x51 // sc_pageDown
            OsCode::KEY_INSERT => (ScanCode::Numpad0, KeyState::E0), // 0x52   // sc_insert
            OsCode::KEY_DELETE => (ScanCode::NumpadPeriod, KeyState::E0), // 0x53   // sc_delete
            OsCode::KEY_LEFTMETA => (ScanCode::Oem2, KeyState::E0), // 0x5B // sc_metaLeft
            OsCode::KEY_RIGHTMETA => (ScanCode::Oem3, KeyState::E0), // 0x5C // sc_metaRight
            OsCode::KEY_FORWARD => (ScanCode::F18, KeyState::E0), // 0x69 // sc_browser_forward
            OsCode::KEY_BACK => (ScanCode::F19, KeyState::E0),   // 0x6A    // sc_browser_back
            OsCode::KEY_COMPOSE => (ScanCode::EraseEOF, KeyState::E0),
            // OsCode::KEY_TODO => 0x24 as ScanCode, // sc_media_stop
            // OsCode::KEY_TODO => 0x32 as ScanCode, // sc_browser_home
            // OsCode::KEY_TODO => 0x46 as ScanCode, // sc_cancel
            // OsCode::KEY_TODO => 0x5D as ScanCode, // sc_application
            // OsCode::KEY_TODO => 0x5E as ScanCode, // sc_power
            // OsCode::KEY_TODO => 0x5F as ScanCode, // sc_sleep
            // OsCode::KEY_TODO => 0x63 as ScanCode, // sc_wake
            // OsCode::KEY_TODO => 0x65 as ScanCode, // sc_browser_search
            // OsCode::KEY_TODO => 0x66 as ScanCode, // sc_browser_favorites
            // OsCode::KEY_TODO => 0x67 as ScanCode, // sc_browser_refresh
            // OsCode::KEY_TODO => 0x68 as ScanCode, // sc_browser_stop
            // 0x6B => OsCode::KEY_TODO, // sc_launch_app1
            // 0x6C => OsCode::KEY_TODO, // sc_launch_email
            // 0x6D => OsCode::KEY_TODO, // sc_launch_media
            _ => return Err(()),
        };
        Ok(Stroke::Keyboard {
            code,
            state,
            information: 0,
        })
    }
}
