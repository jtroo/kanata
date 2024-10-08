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
    AltPrintScreen = 0x54,
    SC_55 = 0x55,
    Int1 = 0x56,
    F11 = 0x57,
    F12 = 0x58,
    SC_59 = 0x59,
    Oem1 = 0x5A,
    Oem2 = 0x5B,
    Oem3 = 0x5C,
    EraseEOF = 0x5D,
    Oem4 = 0x5E,
    Oem5 = 0x5F,
    SC_60 = 0x60,
    SC_61 = 0x61,
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
    Oem6 = 0x6F,
    Katakana = 0x70,
    Oem7 = 0x71,
    SC_72 = 0x72,
    SC_73 = 0x73,
    SC_74 = 0x74,
    SC_75 = 0x75,
    F24 = 0x76,
    SBCSChar = 0x77,
    SC_78 = 0x78,
    Convert = 0x79,
    SC_7A = 0x7A,
    NonConvert = 0x7B,
    SC_7C = 0x7C,
    SC_7D = 0x7D,
    SC_7E = 0x7E,
    SC_7F = 0x7F,
    SC_80 = 0x80,
    SC_81 = 0x81,
    SC_82 = 0x82,
    SC_83 = 0x83,
    SC_84 = 0x84,
    SC_85 = 0x85,
    SC_86 = 0x86,
    SC_87 = 0x87,
    SC_88 = 0x88,
    SC_89 = 0x89,
    SC_8A = 0x8A,
    SC_8B = 0x8B,
    SC_8C = 0x8C,
    SC_8D = 0x8D,
    SC_8E = 0x8E,
    SC_8F = 0x8F,
    SC_90 = 0x90,
    SC_91 = 0x91,
    SC_92 = 0x92,
    SC_93 = 0x93,
    SC_94 = 0x94,
    SC_95 = 0x95,
    SC_96 = 0x96,
    SC_97 = 0x97,
    SC_98 = 0x98,
    SC_99 = 0x99,
    SC_9A = 0x9A,
    SC_9B = 0x9B,
    SC_9C = 0x9C,
    SC_9D = 0x9D,
    SC_9E = 0x9E,
    SC_9F = 0x9F,
    SC_A0 = 0xA0,
    SC_A1 = 0xA1,
    SC_A2 = 0xA2,
    SC_A3 = 0xA3,
    SC_A4 = 0xA4,
    SC_A5 = 0xA5,
    SC_A6 = 0xA6,
    SC_A7 = 0xA7,
    SC_A8 = 0xA8,
    SC_A9 = 0xA9,
    SC_AA = 0xAA,
    SC_AB = 0xAB,
    SC_AC = 0xAC,
    SC_AD = 0xAD,
    SC_AE = 0xAE,
    SC_AF = 0xAF,
    SC_B0 = 0xB0,
    SC_B1 = 0xB1,
    SC_B2 = 0xB2,
    SC_B3 = 0xB3,
    SC_B4 = 0xB4,
    SC_B5 = 0xB5,
    SC_B6 = 0xB6,
    SC_B7 = 0xB7,
    SC_B8 = 0xB8,
    SC_B9 = 0xB9,
    SC_BA = 0xBA,
    SC_BB = 0xBB,
    SC_BC = 0xBC,
    SC_BD = 0xBD,
    SC_BE = 0xBE,
    SC_BF = 0xBF,
    SC_C0 = 0xC0,
    SC_C1 = 0xC1,
    SC_C2 = 0xC2,
    SC_C3 = 0xC3,
    SC_C4 = 0xC4,
    SC_C5 = 0xC5,
    SC_C6 = 0xC6,
    SC_C7 = 0xC7,
    SC_C8 = 0xC8,
    SC_C9 = 0xC9,
    SC_CA = 0xCA,
    SC_CB = 0xCB,
    SC_CC = 0xCC,
    SC_CD = 0xCD,
    SC_CE = 0xCE,
    SC_CF = 0xCF,
    SC_D0 = 0xD0,
    SC_D1 = 0xD1,
    SC_D2 = 0xD2,
    SC_D3 = 0xD3,
    SC_D4 = 0xD4,
    SC_D5 = 0xD5,
    SC_D6 = 0xD6,
    SC_D7 = 0xD7,
    SC_D8 = 0xD8,
    SC_D9 = 0xD9,
    SC_DA = 0xDA,
    SC_DB = 0xDB,
    SC_DC = 0xDC,
    SC_DD = 0xDD,
    SC_DE = 0xDE,
    SC_DF = 0xDF,
    SC_E0 = 0xE0,
    SC_E1 = 0xE1,
    SC_E2 = 0xE2,
    SC_E3 = 0xE3,
    SC_E4 = 0xE4,
    SC_E5 = 0xE5,
    SC_E6 = 0xE6,
    SC_E7 = 0xE7,
    SC_E8 = 0xE8,
    SC_E9 = 0xE9,
    SC_EA = 0xEA,
    SC_EB = 0xEB,
    SC_EC = 0xEC,
    SC_ED = 0xED,
    SC_EE = 0xEE,
    SC_EF = 0xEF,
    SC_F0 = 0xF0,
    SC_F1 = 0xF1,
    SC_F2 = 0xF2,
    SC_F3 = 0xF3,
    SC_F4 = 0xF4,
    SC_F5 = 0xF5,
    SC_F6 = 0xF6,
    SC_F7 = 0xF7,
    SC_F8 = 0xF8,
    SC_F9 = 0xF9,
    SC_FA = 0xFA,
    SC_FB = 0xFB,
    SC_FC = 0xFC,
    SC_FD = 0xFD,
    SC_FE = 0xFE,
    SC_NonExtendMax = 0xFF,
}
