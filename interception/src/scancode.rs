use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

// ref: https://handmade.network/wiki/2823-keyboard_inputs_-_scancodes,_raw_input,_text_input,_key_names
#[derive(Serialize, Deserialize, Hash, Debug, Eq, PartialEq, Copy, Clone, TryFromPrimitive)]
#[repr(u16)]
pub enum ScanCode {
    Esc = 0x01,

    Num1 = 0x02,
    Num2 = 0x03,
    Num3 = 0x04,
    Num4 = 0x05,
    Num5 = 0x06,
    Num6 = 0x07,
    Num7 = 0x08,
    Num8 = 0x09,
    Num9 = 0x0A,
    Num0 = 0x0B,

    Minus = 0x0C,
    Equals = 0x0D,
    Backspace = 0x0E,

    Tab = 0x0F,

    Q = 0x10,
    W = 0x11,
    E = 0x12,
    R = 0x13,
    T = 0x14,
    Y = 0x15,
    U = 0x16,
    I = 0x17,
    O = 0x18,
    P = 0x19,

    LeftBracket = 0x1A,
    RightBracket = 0x1B,
    Enter = 0x1C,

    LeftControl = 0x1D,

    A = 0x1E,
    S = 0x1F,
    D = 0x20,
    F = 0x21,
    G = 0x22,
    H = 0x23,
    J = 0x24,
    K = 0x25,
    L = 0x26,

    SemiColon = 0x27,
    Apostrophe = 0x28,
    Grave = 0x29,
    LeftShift = 0x2A,
    BackSlash = 0x2B,

    Z = 0x2C,
    X = 0x2D,
    C = 0x2E,
    V = 0x2F,
    B = 0x30,
    N = 0x31,
    M = 0x32,

    Comma = 0x33,
    Period = 0x34,
    Slash = 0x35,
    RightShift = 0x36,
    NumpadMultiply = 0x37,
    LeftAlt = 0x38,
    Space = 0x39,
    CapsLock = 0x3A,

    F1 = 0x3B,
    F2 = 0x3C,
    F3 = 0x3D,
    F4 = 0x3E,
    F5 = 0x3F,
    F6 = 0x40,
    F7 = 0x41,
    F8 = 0x42,
    F9 = 0x43,
    F10 = 0x44,

    NumLock = 0x45,
    ScrollLock = 0x46,

    Numpad7 = 0x47,
    Numpad8 = 0x48,
    Numpad9 = 0x49,

    NumpadMinus = 0x4A,

    Numpad4 = 0x4B,
    Numpad5 = 0x4C,
    Numpad6 = 0x4D,

    NumpadPlus = 0x4E,

    Numpad1 = 0x4F,
    Numpad2 = 0x50,
    Numpad3 = 0x51,
    Numpad0 = 0x52,

    NumpadPeriod = 0x53,
    AltPrintScreen = 0x54, /* Alt + print screen. */
    Int1 = 0x56,           /* Key between the left shift and Z. */

    F11 = 0x57,
    F12 = 0x58,

    Oem1 = 0x5A, /* VK_OEM_WSCTRL */
    Oem2 = 0x5B, /* VK_OEM_FINISH */
    Oem3 = 0x5C, /* VK_OEM_JUMP */

    EraseEOF = 0x5D,

    Oem4 = 0x5E, /* VK_OEM_BACKTAB */
    Oem5 = 0x5F, /* VK_OEM_AUTO */

    Zoom = 0x62,
    Help = 0x63,

    F13 = 0x64,
    F14 = 0x65,
    F15 = 0x66,
    F16 = 0x67,
    F17 = 0x68,
    F18 = 0x69,
    F19 = 0x6A,
    F20 = 0x6B,
    F21 = 0x6C,
    F22 = 0x6D,
    F23 = 0x6E,

    Oem6 = 0x6F, /* VK_OEM_PA3 */
    Katakana = 0x70,
    Oem7 = 0x71, /* VK_OEM_RESET */
    F24 = 0x76,

    SBCSChar = 0x77,
    Convert = 0x79,
    NonConvert = 0x7B, /* VK_OEM_PA1 */
}
