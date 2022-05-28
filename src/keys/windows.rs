// This file is adapted from the orginal ktrl's `keys.rs` file for Windows.

use crate::oskbd::*;
use kanata_keyberon::key_code::*;
use std::convert::TryFrom;

// Taken from:
// https://github.com/retep998/winapi-rs/blob/0.3/src/um/winuser.rs#L253
pub const VK_LBUTTON: u32 = 0x01;
pub const VK_RBUTTON: u32 = 0x02;
pub const VK_CANCEL: u32 = 0x03;
pub const VK_MBUTTON: u32 = 0x04;
pub const VK_XBUTTON1: u32 = 0x05;
pub const VK_XBUTTON2: u32 = 0x06;
pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_CLEAR: u32 = 0x0C;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_SHIFT: u32 = 0x10;
pub const VK_CONTROL: u32 = 0x11;
pub const VK_MENU: u32 = 0x12;
pub const VK_PAUSE: u32 = 0x13;
pub const VK_CAPITAL: u32 = 0x14;
pub const VK_KANA: u32 = 0x15;
pub const VK_HANGEUL: u32 = 0x15;
pub const VK_HANGUL: u32 = 0x15;
pub const VK_JUNJA: u32 = 0x17;
pub const VK_FINAL: u32 = 0x18;
pub const VK_HANJA: u32 = 0x19;
pub const VK_KANJI: u32 = 0x19;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_CONVERT: u32 = 0x1C;
pub const VK_NONCONVERT: u32 = 0x1D;
pub const VK_ACCEPT: u32 = 0x1E;
pub const VK_MODECHANGE: u32 = 0x1F;
pub const VK_SPACE: u32 = 0x20;
pub const VK_PRIOR: u32 = 0x21;
pub const VK_NEXT: u32 = 0x22;
pub const VK_END: u32 = 0x23;
pub const VK_HOME: u32 = 0x24;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;
pub const VK_SELECT: u32 = 0x29;
pub const VK_PRINT: u32 = 0x2A;
pub const VK_EXECUTE: u32 = 0x2B;
pub const VK_SNAPSHOT: u32 = 0x2C;
pub const VK_INSERT: u32 = 0x2D;
pub const VK_DELETE: u32 = 0x2E;
pub const VK_HELP: u32 = 0x2F;
pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;
pub const VK_APPS: u32 = 0x5D;
pub const VK_SLEEP: u32 = 0x5F;
pub const VK_NUMPAD0: u32 = 0x60;
pub const VK_NUMPAD1: u32 = 0x61;
pub const VK_NUMPAD2: u32 = 0x62;
pub const VK_NUMPAD3: u32 = 0x63;
pub const VK_NUMPAD4: u32 = 0x64;
pub const VK_NUMPAD5: u32 = 0x65;
pub const VK_NUMPAD6: u32 = 0x66;
pub const VK_NUMPAD7: u32 = 0x67;
pub const VK_NUMPAD8: u32 = 0x68;
pub const VK_NUMPAD9: u32 = 0x69;
pub const VK_MULTIPLY: u32 = 0x6A;
pub const VK_ADD: u32 = 0x6B;
pub const VK_SEPARATOR: u32 = 0x6C;
pub const VK_SUBTRACT: u32 = 0x6D;
pub const VK_DECIMAL: u32 = 0x6E;
pub const VK_DIVIDE: u32 = 0x6F;
pub const VK_F1: u32 = 0x70;
pub const VK_F2: u32 = 0x71;
pub const VK_F3: u32 = 0x72;
pub const VK_F4: u32 = 0x73;
pub const VK_F5: u32 = 0x74;
pub const VK_F6: u32 = 0x75;
pub const VK_F7: u32 = 0x76;
pub const VK_F8: u32 = 0x77;
pub const VK_F9: u32 = 0x78;
pub const VK_F10: u32 = 0x79;
pub const VK_F11: u32 = 0x7A;
pub const VK_F12: u32 = 0x7B;
pub const VK_F13: u32 = 0x7C;
pub const VK_F14: u32 = 0x7D;
pub const VK_F15: u32 = 0x7E;
pub const VK_F16: u32 = 0x7F;
pub const VK_F17: u32 = 0x80;
pub const VK_F18: u32 = 0x81;
pub const VK_F19: u32 = 0x82;
pub const VK_F20: u32 = 0x83;
pub const VK_F21: u32 = 0x84;
pub const VK_F22: u32 = 0x85;
pub const VK_F23: u32 = 0x86;
pub const VK_F24: u32 = 0x87;
pub const VK_NAVIGATION_VIEW: u32 = 0x88;
pub const VK_NAVIGATION_MENU: u32 = 0x89;
pub const VK_NAVIGATION_UP: u32 = 0x8A;
pub const VK_NAVIGATION_DOWN: u32 = 0x8B;
pub const VK_NAVIGATION_LEFT: u32 = 0x8C;
pub const VK_NAVIGATION_RIGHT: u32 = 0x8D;
pub const VK_NAVIGATION_ACCEPT: u32 = 0x8E;
pub const VK_NAVIGATION_CANCEL: u32 = 0x8F;
pub const VK_NUMLOCK: u32 = 0x90;
pub const VK_SCROLL: u32 = 0x91;
pub const VK_OEM_NEC_EQUAL: u32 = 0x92;
pub const VK_OEM_FJ_JISHO: u32 = 0x92;
pub const VK_OEM_FJ_MASSHOU: u32 = 0x93;
pub const VK_OEM_FJ_TOUROKU: u32 = 0x94;
pub const VK_OEM_FJ_LOYA: u32 = 0x95;
pub const VK_OEM_FJ_ROYA: u32 = 0x96;
pub const VK_LSHIFT: u32 = 0xA0;
pub const VK_RSHIFT: u32 = 0xA1;
pub const VK_LCONTROL: u32 = 0xA2;
pub const VK_RCONTROL: u32 = 0xA3;
pub const VK_LMENU: u32 = 0xA4;
pub const VK_RMENU: u32 = 0xA5;
pub const VK_BROWSER_BACK: u32 = 0xA6;
pub const VK_BROWSER_FORWARD: u32 = 0xA7;
pub const VK_BROWSER_REFRESH: u32 = 0xA8;
pub const VK_BROWSER_STOP: u32 = 0xA9;
pub const VK_BROWSER_SEARCH: u32 = 0xAA;
pub const VK_BROWSER_FAVORITES: u32 = 0xAB;
pub const VK_BROWSER_HOME: u32 = 0xAC;
pub const VK_VOLUME_MUTE: u32 = 0xAD;
pub const VK_VOLUME_DOWN: u32 = 0xAE;
pub const VK_VOLUME_UP: u32 = 0xAF;
pub const VK_MEDIA_NEXT_TRACK: u32 = 0xB0;
pub const VK_MEDIA_PREV_TRACK: u32 = 0xB1;
pub const VK_MEDIA_STOP: u32 = 0xB2;
pub const VK_MEDIA_PLAY_PAUSE: u32 = 0xB3;
pub const VK_LAUNCH_MAIL: u32 = 0xB4;
pub const VK_LAUNCH_MEDIA_SELECT: u32 = 0xB5;
pub const VK_LAUNCH_APP1: u32 = 0xB6;
pub const VK_LAUNCH_APP2: u32 = 0xB7;
pub const VK_OEM_1: u32 = 0xBA;
pub const VK_OEM_PLUS: u32 = 0xBB;
pub const VK_OEM_COMMA: u32 = 0xBC;
pub const VK_OEM_MINUS: u32 = 0xBD;
pub const VK_OEM_PERIOD: u32 = 0xBE;
pub const VK_OEM_2: u32 = 0xBF;
pub const VK_OEM_3: u32 = 0xC0;
pub const VK_GAMEPAD_A: u32 = 0xC3;
pub const VK_GAMEPAD_B: u32 = 0xC4;
pub const VK_GAMEPAD_X: u32 = 0xC5;
pub const VK_GAMEPAD_Y: u32 = 0xC6;
pub const VK_GAMEPAD_RIGHT_SHOULDER: u32 = 0xC7;
pub const VK_GAMEPAD_LEFT_SHOULDER: u32 = 0xC8;
pub const VK_GAMEPAD_LEFT_TRIGGER: u32 = 0xC9;
pub const VK_GAMEPAD_RIGHT_TRIGGER: u32 = 0xCA;
pub const VK_GAMEPAD_DPAD_UP: u32 = 0xCB;
pub const VK_GAMEPAD_DPAD_DOWN: u32 = 0xCC;
pub const VK_GAMEPAD_DPAD_LEFT: u32 = 0xCD;
pub const VK_GAMEPAD_DPAD_RIGHT: u32 = 0xCE;
pub const VK_GAMEPAD_MENU: u32 = 0xCF;
pub const VK_GAMEPAD_VIEW: u32 = 0xD0;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON: u32 = 0xD1;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON: u32 = 0xD2;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_UP: u32 = 0xD3;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_DOWN: u32 = 0xD4;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT: u32 = 0xD5;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_LEFT: u32 = 0xD6;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_UP: u32 = 0xD7;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN: u32 = 0xD8;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT: u32 = 0xD9;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT: u32 = 0xDA;
pub const VK_OEM_4: u32 = 0xDB;
pub const VK_OEM_5: u32 = 0xDC;
pub const VK_OEM_6: u32 = 0xDD;
pub const VK_OEM_7: u32 = 0xDE;
pub const VK_OEM_8: u32 = 0xDF;
pub const VK_OEM_AX: u32 = 0xE1;
pub const VK_OEM_102: u32 = 0xE2;
pub const VK_ICO_HELP: u32 = 0xE3;
pub const VK_ICO_00: u32 = 0xE4;
pub const VK_PROCESSKEY: u32 = 0xE5;
pub const VK_ICO_CLEAR: u32 = 0xE6;
pub const VK_PACKET: u32 = 0xE7;
pub const VK_OEM_RESET: u32 = 0xE9;
pub const VK_OEM_JUMP: u32 = 0xEA;
pub const VK_OEM_PA1: u32 = 0xEB;
pub const VK_OEM_PA2: u32 = 0xEC;
pub const VK_OEM_PA3: u32 = 0xED;
pub const VK_OEM_WSCTRL: u32 = 0xEE;
pub const VK_OEM_CUSEL: u32 = 0xEF;
pub const VK_OEM_ATTN: u32 = 0xF0;
pub const VK_OEM_FINISH: u32 = 0xF1;
pub const VK_OEM_COPY: u32 = 0xF2;
pub const VK_OEM_AUTO: u32 = 0xF3;
pub const VK_OEM_ENLW: u32 = 0xF4;
pub const VK_OEM_BACKTAB: u32 = 0xF5;
pub const VK_ATTN: u32 = 0xF6;
pub const VK_CRSEL: u32 = 0xF7;
pub const VK_EXSEL: u32 = 0xF8;
pub const VK_EREOF: u32 = 0xF9;
pub const VK_PLAY: u32 = 0xFA;
pub const VK_ZOOM: u32 = 0xFB;
pub const VK_NONAME: u32 = 0xFC;
pub const VK_PA1: u32 = 0xFD;
pub const VK_OEM_CLEAR: u32 = 0xFE;

/// This is a shameless copy of evdev_rs::enums::EV_KEY.
/// I've added the Copy trait and I'll be able
/// to added my own Impl(s) to it
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OsCode {
    KEY_RESERVED,
    KEY_ESC,
    KEY_1,
    KEY_2,
    KEY_3,
    KEY_4,
    KEY_5,
    KEY_6,
    KEY_7,
    KEY_8,
    KEY_9,
    KEY_0,
    KEY_MINUS,
    KEY_EQUAL,
    KEY_BACKSPACE,
    KEY_TAB,
    KEY_Q,
    KEY_W,
    KEY_E,
    KEY_R,
    KEY_T,
    KEY_Y,
    KEY_U,
    KEY_I,
    KEY_O,
    KEY_P,
    KEY_LEFTBRACE,
    KEY_RIGHTBRACE,
    KEY_ENTER,
    KEY_LEFTCTRL,
    KEY_A,
    KEY_S,
    KEY_D,
    KEY_F,
    KEY_G,
    KEY_H,
    KEY_J,
    KEY_K,
    KEY_L,
    KEY_SEMICOLON,
    KEY_APOSTROPHE,
    KEY_GRAVE,
    KEY_LEFTSHIFT,
    KEY_BACKSLASH,
    KEY_Z,
    KEY_X,
    KEY_C,
    KEY_V,
    KEY_B,
    KEY_N,
    KEY_M,
    KEY_COMMA,
    KEY_DOT,
    KEY_SLASH,
    KEY_RIGHTSHIFT,
    KEY_KPASTERISK,
    KEY_LEFTALT,
    KEY_SPACE,
    KEY_CAPSLOCK,
    KEY_F1,
    KEY_F2,
    KEY_F3,
    KEY_F4,
    KEY_F5,
    KEY_F6,
    KEY_F7,
    KEY_F8,
    KEY_F9,
    KEY_F10,
    KEY_NUMLOCK,
    KEY_SCROLLLOCK,
    KEY_KP7,
    KEY_KP8,
    KEY_KP9,
    KEY_KPMINUS,
    KEY_KP4,
    KEY_KP5,
    KEY_KP6,
    KEY_KPPLUS,
    KEY_KP1,
    KEY_KP2,
    KEY_KP3,
    KEY_KP0,
    KEY_KPDOT,
    KEY_ZENKAKUHANKAKU,
    KEY_102ND,
    KEY_F11,
    KEY_F12,
    KEY_RO,
    KEY_KATAKANA,
    KEY_HIRAGANA,
    KEY_HENKAN,
    KEY_KATAKANAHIRAGANA,
    KEY_MUHENKAN,
    KEY_KPJPCOMMA,
    KEY_KPENTER,
    KEY_RIGHTCTRL,
    KEY_KPSLASH,
    KEY_SYSRQ,
    KEY_RIGHTALT,
    KEY_LINEFEED,
    KEY_HOME,
    KEY_UP,
    KEY_PAGEUP,
    KEY_LEFT,
    KEY_RIGHT,
    KEY_END,
    KEY_DOWN,
    KEY_PAGEDOWN,
    KEY_INSERT,
    KEY_DELETE,
    KEY_MACRO,
    KEY_MUTE,
    KEY_VOLUMEDOWN,
    KEY_VOLUMEUP,
    KEY_POWER,
    KEY_KPEQUAL,
    KEY_KPPLUSMINUS,
    KEY_PAUSE,
    KEY_SCALE,
    KEY_KPCOMMA,
    KEY_HANGEUL,
    KEY_HANJA,
    KEY_YEN,
    KEY_LEFTMETA,
    KEY_RIGHTMETA,
    KEY_COMPOSE,
    KEY_STOP,
    KEY_AGAIN,
    KEY_PROPS,
    KEY_UNDO,
    KEY_FRONT,
    KEY_COPY,
    KEY_OPEN,
    KEY_PASTE,
    KEY_FIND,
    KEY_CUT,
    KEY_HELP,
    KEY_MENU,
    KEY_CALC,
    KEY_SETUP,
    KEY_SLEEP,
    KEY_WAKEUP,
    KEY_FILE,
    KEY_SENDFILE,
    KEY_DELETEFILE,
    KEY_XFER,
    KEY_PROG1,
    KEY_PROG2,
    KEY_WWW,
    KEY_MSDOS,
    KEY_COFFEE,
    KEY_ROTATE_DISPLAY,
    KEY_CYCLEWINDOWS,
    KEY_MAIL,
    KEY_BOOKMARKS,
    KEY_COMPUTER,
    KEY_BACK,
    KEY_FORWARD,
    KEY_CLOSECD,
    KEY_EJECTCD,
    KEY_EJECTCLOSECD,
    KEY_NEXTSONG,
    KEY_PLAYPAUSE,
    KEY_PREVIOUSSONG,
    KEY_STOPCD,
    KEY_RECORD,
    KEY_REWIND,
    KEY_PHONE,
    KEY_ISO,
    KEY_CONFIG,
    KEY_HOMEPAGE,
    KEY_REFRESH,
    KEY_EXIT,
    KEY_MOVE,
    KEY_EDIT,
    KEY_SCROLLUP,
    KEY_SCROLLDOWN,
    KEY_KPLEFTPAREN,
    KEY_KPRIGHTPAREN,
    KEY_NEW,
    KEY_REDO,
    KEY_F13,
    KEY_F14,
    KEY_F15,
    KEY_F16,
    KEY_F17,
    KEY_F18,
    KEY_F19,
    KEY_F20,
    KEY_F21,
    KEY_F22,
    KEY_F23,
    KEY_F24,
    KEY_PLAYCD,
    KEY_PAUSECD,
    KEY_PROG3,
    KEY_PROG4,
    KEY_DASHBOARD,
    KEY_SUSPEND,
    KEY_CLOSE,
    KEY_PLAY,
    KEY_FASTFORWARD,
    KEY_BASSBOOST,
    KEY_PRINT,
    KEY_HP,
    KEY_CAMERA,
    KEY_SOUND,
    KEY_QUESTION,
    KEY_EMAIL,
    KEY_CHAT,
    KEY_SEARCH,
    KEY_CONNECT,
    KEY_FINANCE,
    KEY_SPORT,
    KEY_SHOP,
    KEY_ALTERASE,
    KEY_CANCEL,
    KEY_BRIGHTNESSDOWN,
    KEY_BRIGHTNESSUP,
    KEY_MEDIA,
    KEY_SWITCHVIDEOMODE,
    KEY_KBDILLUMTOGGLE,
    KEY_KBDILLUMDOWN,
    KEY_KBDILLUMUP,
    KEY_SEND,
    KEY_REPLY,
    KEY_FORWARDMAIL,
    KEY_SAVE,
    KEY_DOCUMENTS,
    KEY_BATTERY,
    KEY_BLUETOOTH,
    KEY_WLAN,
    KEY_UWB,
    KEY_UNKNOWN,
    KEY_VIDEO_NEXT,
    KEY_VIDEO_PREV,
    KEY_BRIGHTNESS_CYCLE,
    KEY_BRIGHTNESS_AUTO,
    KEY_DISPLAY_OFF,
    KEY_WWAN,
    KEY_RFKILL,
    KEY_MICMUTE,
    KEY_OK,
    KEY_SELECT,
    KEY_GOTO,
    KEY_CLEAR,
    KEY_POWER2,
    KEY_OPTION,
    KEY_INFO,
    KEY_TIME,
    KEY_VENDOR,
    KEY_ARCHIVE,
    KEY_PROGRAM,
    KEY_CHANNEL,
    KEY_FAVORITES,
    KEY_EPG,
    KEY_PVR,
    KEY_MHP,
    KEY_LANGUAGE,
    KEY_TITLE,
    KEY_SUBTITLE,
    KEY_ANGLE,
    KEY_FULL_SCREEN,
    KEY_MODE,
    KEY_KEYBOARD,
    KEY_ASPECT_RATIO,
    KEY_PC,
    KEY_TV,
    KEY_TV2,
    KEY_VCR,
    KEY_VCR2,
    KEY_SAT,
    KEY_SAT2,
    KEY_CD,
    KEY_TAPE,
    KEY_RADIO,
    KEY_TUNER,
    KEY_PLAYER,
    KEY_TEXT,
    KEY_DVD,
    KEY_AUX,
    KEY_MP3,
    KEY_AUDIO,
    KEY_VIDEO,
    KEY_DIRECTORY,
    KEY_LIST,
    KEY_MEMO,
    KEY_CALENDAR,
    KEY_RED,
    KEY_GREEN,
    KEY_YELLOW,
    KEY_BLUE,
    KEY_CHANNELUP,
    KEY_CHANNELDOWN,
    KEY_FIRST,
    KEY_LAST,
    KEY_AB,
    KEY_NEXT,
    KEY_RESTART,
    KEY_SLOW,
    KEY_SHUFFLE,
    KEY_BREAK,
    KEY_PREVIOUS,
    KEY_DIGITS,
    KEY_TEEN,
    KEY_TWEN,
    KEY_VIDEOPHONE,
    KEY_GAMES,
    KEY_ZOOMIN,
    KEY_ZOOMOUT,
    KEY_ZOOMRESET,
    KEY_WORDPROCESSOR,
    KEY_EDITOR,
    KEY_SPREADSHEET,
    KEY_GRAPHICSEDITOR,
    KEY_PRESENTATION,
    KEY_DATABASE,
    KEY_NEWS,
    KEY_VOICEMAIL,
    KEY_ADDRESSBOOK,
    KEY_MESSENGER,
    KEY_DISPLAYTOGGLE,
    KEY_SPELLCHECK,
    KEY_LOGOFF,
    KEY_DOLLAR,
    KEY_EURO,
    KEY_FRAMEBACK,
    KEY_FRAMEFORWARD,
    KEY_CONTEXT_MENU,
    KEY_MEDIA_REPEAT,
    KEY_10CHANNELSUP,
    KEY_10CHANNELSDOWN,
    KEY_IMAGES,
    KEY_DEL_EOL,
    KEY_DEL_EOS,
    KEY_INS_LINE,
    KEY_DEL_LINE,
    KEY_FN,
    KEY_FN_ESC,
    KEY_FN_F1,
    KEY_FN_F2,
    KEY_FN_F3,
    KEY_FN_F4,
    KEY_FN_F5,
    KEY_FN_F6,
    KEY_FN_F7,
    KEY_FN_F8,
    KEY_FN_F9,
    KEY_FN_F10,
    KEY_FN_F11,
    KEY_FN_F12,
    KEY_FN_1,
    KEY_FN_2,
    KEY_FN_D,
    KEY_FN_E,
    KEY_FN_F,
    KEY_FN_S,
    KEY_FN_B,
    KEY_BRL_DOT1,
    KEY_BRL_DOT2,
    KEY_BRL_DOT3,
    KEY_BRL_DOT4,
    KEY_BRL_DOT5,
    KEY_BRL_DOT6,
    KEY_BRL_DOT7,
    KEY_BRL_DOT8,
    KEY_BRL_DOT9,
    KEY_BRL_DOT10,
    KEY_NUMERIC_0,
    KEY_NUMERIC_1,
    KEY_NUMERIC_2,
    KEY_NUMERIC_3,
    KEY_NUMERIC_4,
    KEY_NUMERIC_5,
    KEY_NUMERIC_6,
    KEY_NUMERIC_7,
    KEY_NUMERIC_8,
    KEY_NUMERIC_9,
    KEY_NUMERIC_STAR,
    KEY_NUMERIC_POUND,
    KEY_NUMERIC_A,
    KEY_NUMERIC_B,
    KEY_NUMERIC_C,
    KEY_NUMERIC_D,
    KEY_CAMERA_FOCUS,
    KEY_WPS_BUTTON,
    KEY_TOUCHPAD_TOGGLE,
    KEY_TOUCHPAD_ON,
    KEY_TOUCHPAD_OFF,
    KEY_CAMERA_ZOOMIN,
    KEY_CAMERA_ZOOMOUT,
    KEY_CAMERA_UP,
    KEY_CAMERA_DOWN,
    KEY_CAMERA_LEFT,
    KEY_CAMERA_RIGHT,
    KEY_ATTENDANT_ON,
    KEY_ATTENDANT_OFF,
    KEY_ATTENDANT_TOGGLE,
    KEY_LIGHTS_TOGGLE,
    KEY_ALS_TOGGLE,
    KEY_ROTATE_LOCK_TOGGLE,
    KEY_BUTTONCONFIG,
    KEY_TASKMANAGER,
    KEY_JOURNAL,
    KEY_CONTROLPANEL,
    KEY_APPSELECT,
    KEY_SCREENSAVER,
    KEY_VOICECOMMAND,
    KEY_ASSISTANT,
    KEY_KBD_LAYOUT_NEXT,
    KEY_BRIGHTNESS_MIN,
    KEY_BRIGHTNESS_MAX,
    KEY_KBDINPUTASSIST_PREV,
    KEY_KBDINPUTASSIST_NEXT,
    KEY_KBDINPUTASSIST_PREVGROUP,
    KEY_KBDINPUTASSIST_NEXTGROUP,
    KEY_KBDINPUTASSIST_ACCEPT,
    KEY_KBDINPUTASSIST_CANCEL,
    KEY_RIGHT_UP,
    KEY_RIGHT_DOWN,
    KEY_LEFT_UP,
    KEY_LEFT_DOWN,
    KEY_ROOT_MENU,
    KEY_MEDIA_TOP_MENU,
    KEY_NUMERIC_11,
    KEY_NUMERIC_12,
    KEY_AUDIO_DESC,
    KEY_3D_MODE,
    KEY_NEXT_FAVORITE,
    KEY_STOP_RECORD,
    KEY_PAUSE_RECORD,
    KEY_VOD,
    KEY_UNMUTE,
    KEY_FASTREVERSE,
    KEY_SLOWREVERSE,
    KEY_DATA,
    KEY_ONSCREEN_KEYBOARD,
    KEY_MAX,
    BTN_0,
    BTN_1,
    BTN_2,
    BTN_3,
    BTN_4,
    BTN_5,
    BTN_6,
    BTN_7,
    BTN_8,
    BTN_9,
    BTN_LEFT,
    BTN_RIGHT,
    BTN_MIDDLE,
    BTN_SIDE,
    BTN_EXTRA,
    BTN_FORWARD,
    BTN_BACK,
    BTN_TASK,
    BTN_TRIGGER,
    BTN_THUMB,
    BTN_THUMB2,
    BTN_TOP,
    BTN_TOP2,
    BTN_PINKIE,
    BTN_BASE,
    BTN_BASE2,
    BTN_BASE3,
    BTN_BASE4,
    BTN_BASE5,
    BTN_BASE6,
    BTN_DEAD,
    BTN_SOUTH,
    BTN_EAST,
    BTN_C,
    BTN_NORTH,
    BTN_WEST,
    BTN_Z,
    BTN_TL,
    BTN_TR,
    BTN_TL2,
    BTN_TR2,
    BTN_SELECT,
    BTN_START,
    BTN_MODE,
    BTN_THUMBL,
    BTN_THUMBR,
    BTN_TOOL_PEN,
    BTN_TOOL_RUBBER,
    BTN_TOOL_BRUSH,
    BTN_TOOL_PENCIL,
    BTN_TOOL_AIRBRUSH,
    BTN_TOOL_FINGER,
    BTN_TOOL_MOUSE,
    BTN_TOOL_LENS,
    BTN_TOOL_QUINTTAP,
    BTN_STYLUS3,
    BTN_TOUCH,
    BTN_STYLUS,
    BTN_STYLUS2,
    BTN_TOOL_DOUBLETAP,
    BTN_TOOL_TRIPLETAP,
    BTN_TOOL_QUADTAP,
    BTN_GEAR_DOWN,
    BTN_GEAR_UP,
    BTN_DPAD_UP,
    BTN_DPAD_DOWN,
    BTN_DPAD_LEFT,
    BTN_DPAD_RIGHT,
    BTN_TRIGGER_HAPPY1,
    BTN_TRIGGER_HAPPY2,
    BTN_TRIGGER_HAPPY3,
    BTN_TRIGGER_HAPPY4,
    BTN_TRIGGER_HAPPY5,
    BTN_TRIGGER_HAPPY6,
    BTN_TRIGGER_HAPPY7,
    BTN_TRIGGER_HAPPY8,
    BTN_TRIGGER_HAPPY9,
    BTN_TRIGGER_HAPPY10,
    BTN_TRIGGER_HAPPY11,
    BTN_TRIGGER_HAPPY12,
    BTN_TRIGGER_HAPPY13,
    BTN_TRIGGER_HAPPY14,
    BTN_TRIGGER_HAPPY15,
    BTN_TRIGGER_HAPPY16,
    BTN_TRIGGER_HAPPY17,
    BTN_TRIGGER_HAPPY18,
    BTN_TRIGGER_HAPPY19,
    BTN_TRIGGER_HAPPY20,
    BTN_TRIGGER_HAPPY21,
    BTN_TRIGGER_HAPPY22,
    BTN_TRIGGER_HAPPY23,
    BTN_TRIGGER_HAPPY24,
    BTN_TRIGGER_HAPPY25,
    BTN_TRIGGER_HAPPY26,
    BTN_TRIGGER_HAPPY27,
    BTN_TRIGGER_HAPPY28,
    BTN_TRIGGER_HAPPY29,
    BTN_TRIGGER_HAPPY30,
    BTN_TRIGGER_HAPPY31,
    BTN_TRIGGER_HAPPY32,
    BTN_TRIGGER_HAPPY33,
    BTN_TRIGGER_HAPPY34,
    BTN_TRIGGER_HAPPY35,
    BTN_TRIGGER_HAPPY36,
    BTN_TRIGGER_HAPPY37,
    BTN_TRIGGER_HAPPY38,
    BTN_TRIGGER_HAPPY39,
    BTN_TRIGGER_HAPPY40,
    BTN_MAX,
}

impl OsCode {
    pub fn from_u32(code: u32) -> Option<Self> {
        match code {
            0x30 => Some(OsCode::KEY_0),
            0x31 => Some(OsCode::KEY_1),
            0x32 => Some(OsCode::KEY_2),
            0x33 => Some(OsCode::KEY_3),
            0x34 => Some(OsCode::KEY_4),
            0x35 => Some(OsCode::KEY_5),
            0x36 => Some(OsCode::KEY_6),
            0x37 => Some(OsCode::KEY_7),
            0x38 => Some(OsCode::KEY_8),
            0x39 => Some(OsCode::KEY_9),
            0x41 => Some(OsCode::KEY_A),
            0x42 => Some(OsCode::KEY_B),
            0x43 => Some(OsCode::KEY_C),
            0x44 => Some(OsCode::KEY_D),
            0x45 => Some(OsCode::KEY_E),
            0x46 => Some(OsCode::KEY_F),
            0x47 => Some(OsCode::KEY_G),
            0x48 => Some(OsCode::KEY_H),
            0x49 => Some(OsCode::KEY_I),
            0x4A => Some(OsCode::KEY_J),
            0x4B => Some(OsCode::KEY_K),
            0x4C => Some(OsCode::KEY_L),
            0x4D => Some(OsCode::KEY_M),
            0x4E => Some(OsCode::KEY_N),
            0x4F => Some(OsCode::KEY_O),
            0x50 => Some(OsCode::KEY_P),
            0x51 => Some(OsCode::KEY_Q),
            0x52 => Some(OsCode::KEY_R),
            0x53 => Some(OsCode::KEY_S),
            0x54 => Some(OsCode::KEY_T),
            0x55 => Some(OsCode::KEY_U),
            0x56 => Some(OsCode::KEY_V),
            0x57 => Some(OsCode::KEY_W),
            0x58 => Some(OsCode::KEY_X),
            0x59 => Some(OsCode::KEY_Y),
            0x5A => Some(OsCode::KEY_Z),
            VK_OEM_1 => Some(OsCode::KEY_SEMICOLON),
            VK_OEM_2 => Some(OsCode::KEY_SLASH),
            VK_OEM_3 => Some(OsCode::KEY_GRAVE),
            VK_OEM_4 => Some(OsCode::KEY_LEFTBRACE),
            VK_OEM_5 => Some(OsCode::KEY_BACKSLASH),
            VK_OEM_6 => Some(OsCode::KEY_RIGHTBRACE),
            VK_OEM_7 => Some(OsCode::KEY_APOSTROPHE),
            VK_OEM_MINUS => Some(OsCode::KEY_MINUS),
            VK_OEM_PERIOD => Some(OsCode::KEY_DOT),
            VK_OEM_PLUS => Some(OsCode::KEY_EQUAL),
            VK_BACK => Some(OsCode::KEY_BACKSPACE),
            VK_ESCAPE => Some(OsCode::KEY_ESC),
            VK_TAB => Some(OsCode::KEY_TAB),
            VK_RETURN => Some(OsCode::KEY_ENTER),
            VK_LCONTROL => Some(OsCode::KEY_LEFTCTRL),
            VK_LSHIFT => Some(OsCode::KEY_LEFTSHIFT),
            VK_OEM_COMMA => Some(OsCode::KEY_COMMA),
            VK_RSHIFT => Some(OsCode::KEY_RIGHTSHIFT),
            VK_MULTIPLY => Some(OsCode::KEY_KPASTERISK),
            VK_LMENU => Some(OsCode::KEY_LEFTALT),
            VK_SPACE => Some(OsCode::KEY_SPACE),
            VK_CAPITAL => Some(OsCode::KEY_CAPSLOCK),
            VK_F1 => Some(OsCode::KEY_F1),
            VK_F2 => Some(OsCode::KEY_F2),
            VK_F3 => Some(OsCode::KEY_F3),
            VK_F4 => Some(OsCode::KEY_F4),
            VK_F5 => Some(OsCode::KEY_F5),
            VK_F6 => Some(OsCode::KEY_F6),
            VK_F7 => Some(OsCode::KEY_F7),
            VK_F8 => Some(OsCode::KEY_F8),
            VK_F9 => Some(OsCode::KEY_F9),
            VK_F10 => Some(OsCode::KEY_F10),
            VK_F11 => Some(OsCode::KEY_F11),
            VK_F12 => Some(OsCode::KEY_F12),
            VK_NUMLOCK => Some(OsCode::KEY_NUMLOCK),
            VK_SCROLL => Some(OsCode::KEY_SCROLLLOCK),
            VK_NUMPAD0 => Some(OsCode::KEY_KP0),
            VK_NUMPAD1 => Some(OsCode::KEY_KP1),
            VK_NUMPAD2 => Some(OsCode::KEY_KP2),
            VK_NUMPAD3 => Some(OsCode::KEY_KP3),
            VK_NUMPAD4 => Some(OsCode::KEY_KP4),
            VK_NUMPAD5 => Some(OsCode::KEY_KP5),
            VK_NUMPAD6 => Some(OsCode::KEY_KP6),
            VK_NUMPAD7 => Some(OsCode::KEY_KP7),
            VK_NUMPAD8 => Some(OsCode::KEY_KP8),
            VK_NUMPAD9 => Some(OsCode::KEY_KP9),
            VK_SUBTRACT => Some(OsCode::KEY_KPMINUS),
            VK_ADD => Some(OsCode::KEY_KPPLUS),
            VK_DECIMAL => Some(OsCode::KEY_KPDOT),
            VK_RCONTROL => Some(OsCode::KEY_RIGHTCTRL),
            VK_DIVIDE => Some(OsCode::KEY_KPSLASH),
            VK_RMENU => Some(OsCode::KEY_RIGHTALT),
            VK_HOME => Some(OsCode::KEY_HOME),
            VK_UP => Some(OsCode::KEY_UP),
            VK_PRIOR => Some(OsCode::KEY_PAGEUP),
            VK_LEFT => Some(OsCode::KEY_LEFT),
            VK_RIGHT => Some(OsCode::KEY_RIGHT),
            VK_END => Some(OsCode::KEY_END),
            VK_DOWN => Some(OsCode::KEY_DOWN),
            VK_NEXT => Some(OsCode::KEY_PAGEDOWN),
            VK_INSERT => Some(OsCode::KEY_INSERT),
            VK_DELETE => Some(OsCode::KEY_DELETE),
            VK_VOLUME_MUTE => Some(OsCode::KEY_MUTE),
            VK_VOLUME_DOWN => Some(OsCode::KEY_VOLUMEDOWN),
            VK_VOLUME_UP => Some(OsCode::KEY_VOLUMEUP),
            VK_PAUSE => Some(OsCode::KEY_PAUSE),
            VK_LWIN => Some(OsCode::KEY_LEFTMETA),
            VK_RWIN => Some(OsCode::KEY_RIGHTMETA),
            VK_BROWSER_BACK => Some(OsCode::KEY_BACK),
            VK_BROWSER_FORWARD => Some(OsCode::KEY_FORWARD),
            VK_MEDIA_NEXT_TRACK => Some(OsCode::KEY_NEXTSONG),
            VK_MEDIA_PLAY_PAUSE => Some(OsCode::KEY_PLAYPAUSE),
            VK_MEDIA_PREV_TRACK => Some(OsCode::KEY_PREVIOUSSONG),
            VK_MEDIA_STOP => Some(OsCode::KEY_STOP),
            VK_BROWSER_HOME => Some(OsCode::KEY_HOMEPAGE),
            VK_BROWSER_REFRESH => Some(OsCode::KEY_REFRESH),
            VK_F13 => Some(OsCode::KEY_F13),
            VK_F14 => Some(OsCode::KEY_F14),
            VK_F15 => Some(OsCode::KEY_F15),
            VK_F16 => Some(OsCode::KEY_F16),
            VK_F17 => Some(OsCode::KEY_F17),
            VK_F18 => Some(OsCode::KEY_F18),
            VK_F19 => Some(OsCode::KEY_F19),
            VK_F20 => Some(OsCode::KEY_F20),
            VK_F21 => Some(OsCode::KEY_F21),
            VK_F22 => Some(OsCode::KEY_F22),
            VK_F23 => Some(OsCode::KEY_F23),
            VK_F24 => Some(OsCode::KEY_F24),
            VK_HANGEUL => Some(OsCode::KEY_HANGEUL),
            VK_HANJA => Some(OsCode::KEY_HANJA),
            VK_PLAY => Some(OsCode::KEY_PLAY),
            VK_PRINT => Some(OsCode::KEY_PRINT),
            VK_BROWSER_SEARCH => Some(OsCode::KEY_SEARCH),
            VK_BROWSER_FAVORITES => Some(OsCode::KEY_FAVORITES),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            OsCode::KEY_0 => 0x30,
            OsCode::KEY_1 => 0x31,
            OsCode::KEY_2 => 0x32,
            OsCode::KEY_3 => 0x33,
            OsCode::KEY_4 => 0x34,
            OsCode::KEY_5 => 0x35,
            OsCode::KEY_6 => 0x36,
            OsCode::KEY_7 => 0x37,
            OsCode::KEY_8 => 0x38,
            OsCode::KEY_9 => 0x39,
            OsCode::KEY_A => 0x41,
            OsCode::KEY_B => 0x42,
            OsCode::KEY_C => 0x43,
            OsCode::KEY_D => 0x44,
            OsCode::KEY_E => 0x45,
            OsCode::KEY_F => 0x46,
            OsCode::KEY_G => 0x47,
            OsCode::KEY_H => 0x48,
            OsCode::KEY_I => 0x49,
            OsCode::KEY_J => 0x4A,
            OsCode::KEY_K => 0x4B,
            OsCode::KEY_L => 0x4C,
            OsCode::KEY_M => 0x4D,
            OsCode::KEY_N => 0x4E,
            OsCode::KEY_O => 0x4F,
            OsCode::KEY_P => 0x50,
            OsCode::KEY_Q => 0x51,
            OsCode::KEY_R => 0x52,
            OsCode::KEY_S => 0x53,
            OsCode::KEY_T => 0x54,
            OsCode::KEY_U => 0x55,
            OsCode::KEY_V => 0x56,
            OsCode::KEY_W => 0x57,
            OsCode::KEY_X => 0x58,
            OsCode::KEY_Y => 0x59,
            OsCode::KEY_Z => 0x5A,
            OsCode::KEY_SEMICOLON => VK_OEM_1,
            OsCode::KEY_SLASH => VK_OEM_2,
            OsCode::KEY_GRAVE => VK_OEM_3,
            OsCode::KEY_LEFTBRACE => VK_OEM_4,
            OsCode::KEY_BACKSLASH => VK_OEM_5,
            OsCode::KEY_RIGHTBRACE => VK_OEM_6,
            OsCode::KEY_APOSTROPHE => VK_OEM_7,
            OsCode::KEY_MINUS => VK_OEM_MINUS,
            OsCode::KEY_DOT => VK_OEM_PERIOD,
            OsCode::KEY_EQUAL => VK_OEM_PLUS,
            OsCode::KEY_BACKSPACE => VK_BACK,
            OsCode::KEY_ESC => VK_ESCAPE,
            OsCode::KEY_TAB => VK_TAB,
            OsCode::KEY_ENTER => VK_RETURN,
            OsCode::KEY_LEFTCTRL => VK_LCONTROL,
            OsCode::KEY_LEFTSHIFT => VK_LSHIFT,
            OsCode::KEY_COMMA => VK_OEM_COMMA,
            OsCode::KEY_RIGHTSHIFT => VK_RSHIFT,
            OsCode::KEY_KPASTERISK => VK_MULTIPLY,
            OsCode::KEY_LEFTALT => VK_LMENU,
            OsCode::KEY_SPACE => VK_SPACE,
            OsCode::KEY_CAPSLOCK => VK_CAPITAL,
            OsCode::KEY_F1 => VK_F1,
            OsCode::KEY_F2 => VK_F2,
            OsCode::KEY_F3 => VK_F3,
            OsCode::KEY_F4 => VK_F4,
            OsCode::KEY_F5 => VK_F5,
            OsCode::KEY_F6 => VK_F6,
            OsCode::KEY_F7 => VK_F7,
            OsCode::KEY_F8 => VK_F8,
            OsCode::KEY_F9 => VK_F9,
            OsCode::KEY_F10 => VK_F10,
            OsCode::KEY_F11 => VK_F11,
            OsCode::KEY_F12 => VK_F12,
            OsCode::KEY_NUMLOCK => VK_NUMLOCK,
            OsCode::KEY_SCROLLLOCK => VK_SCROLL,
            OsCode::KEY_KP0 => VK_NUMPAD0,
            OsCode::KEY_KP1 => VK_NUMPAD1,
            OsCode::KEY_KP2 => VK_NUMPAD2,
            OsCode::KEY_KP3 => VK_NUMPAD3,
            OsCode::KEY_KP4 => VK_NUMPAD4,
            OsCode::KEY_KP5 => VK_NUMPAD5,
            OsCode::KEY_KP6 => VK_NUMPAD6,
            OsCode::KEY_KP7 => VK_NUMPAD7,
            OsCode::KEY_KP8 => VK_NUMPAD8,
            OsCode::KEY_KP9 => VK_NUMPAD9,
            OsCode::KEY_KPMINUS => VK_SUBTRACT,
            OsCode::KEY_KPPLUS => VK_ADD,
            OsCode::KEY_KPDOT => VK_DECIMAL,
            OsCode::KEY_RIGHTCTRL => VK_RCONTROL,
            OsCode::KEY_KPSLASH => VK_DIVIDE,
            OsCode::KEY_RIGHTALT => VK_RMENU,
            OsCode::KEY_HOME => VK_HOME,
            OsCode::KEY_UP => VK_UP,
            OsCode::KEY_PAGEUP => VK_PRIOR,
            OsCode::KEY_LEFT => VK_LEFT,
            OsCode::KEY_RIGHT => VK_RIGHT,
            OsCode::KEY_END => VK_END,
            OsCode::KEY_DOWN => VK_DOWN,
            OsCode::KEY_PAGEDOWN => VK_NEXT,
            OsCode::KEY_INSERT => VK_INSERT,
            OsCode::KEY_DELETE => VK_DELETE,
            OsCode::KEY_MUTE => VK_VOLUME_MUTE,
            OsCode::KEY_VOLUMEDOWN => VK_VOLUME_DOWN,
            OsCode::KEY_VOLUMEUP => VK_VOLUME_UP,
            OsCode::KEY_PAUSE => VK_PAUSE,
            OsCode::KEY_LEFTMETA => VK_LWIN,
            OsCode::KEY_RIGHTMETA => VK_RWIN,
            OsCode::KEY_BACK => VK_BROWSER_BACK,
            OsCode::KEY_FORWARD => VK_BROWSER_FORWARD,
            OsCode::KEY_NEXTSONG => VK_MEDIA_NEXT_TRACK,
            OsCode::KEY_PLAYPAUSE => VK_MEDIA_PLAY_PAUSE,
            OsCode::KEY_PREVIOUSSONG => VK_MEDIA_PREV_TRACK,
            OsCode::KEY_STOP => VK_MEDIA_STOP,
            OsCode::KEY_HOMEPAGE => VK_BROWSER_HOME,
            OsCode::KEY_REFRESH => VK_BROWSER_REFRESH,
            OsCode::KEY_F13 => VK_F13,
            OsCode::KEY_F14 => VK_F14,
            OsCode::KEY_F15 => VK_F15,
            OsCode::KEY_F16 => VK_F16,
            OsCode::KEY_F17 => VK_F17,
            OsCode::KEY_F18 => VK_F18,
            OsCode::KEY_F19 => VK_F19,
            OsCode::KEY_F20 => VK_F20,
            OsCode::KEY_F21 => VK_F21,
            OsCode::KEY_F22 => VK_F22,
            OsCode::KEY_F23 => VK_F23,
            OsCode::KEY_F24 => VK_F24,
            OsCode::KEY_HANGEUL => VK_HANGEUL,
            OsCode::KEY_HANJA => VK_HANJA,
            OsCode::KEY_PLAY => VK_PLAY,
            OsCode::KEY_PRINT => VK_PRINT,
            OsCode::KEY_SEARCH => VK_BROWSER_SEARCH,
            OsCode::KEY_FAVORITES => VK_BROWSER_FAVORITES,
            _ => 0,
        }
    }
}

impl TryFrom<usize> for OsCode {
    type Error = ();
    fn try_from(item: usize) -> Result<Self, Self::Error> {
        match Self::from_u32(item as u32) {
            Some(kc) => Ok(kc),
            _ => Err(()),
        }
    }
}

impl From<u32> for OsCode {
    fn from(item: u32) -> Self {
        Self::from_u32(item).unwrap_or_else(|| panic!("Invalid KeyCode: {}", item))
    }
}

impl From<OsCode> for usize {
    fn from(item: OsCode) -> Self {
        item.as_u32() as usize
    }
}

impl From<OsCode> for u32 {
    fn from(item: OsCode) -> Self {
        item.as_u32()
    }
}

impl From<KeyCode> for OsCode {
    fn from(item: KeyCode) -> Self {
        match item {
            KeyCode::Escape => OsCode::KEY_ESC,
            KeyCode::Kb1 => OsCode::KEY_1,
            KeyCode::Kb2 => OsCode::KEY_2,
            KeyCode::Kb3 => OsCode::KEY_3,
            KeyCode::Kb4 => OsCode::KEY_4,
            KeyCode::Kb5 => OsCode::KEY_5,
            KeyCode::Kb6 => OsCode::KEY_6,
            KeyCode::Kb7 => OsCode::KEY_7,
            KeyCode::Kb8 => OsCode::KEY_8,
            KeyCode::Kb9 => OsCode::KEY_9,
            KeyCode::Kb0 => OsCode::KEY_0,
            KeyCode::Minus => OsCode::KEY_MINUS,
            KeyCode::Equal => OsCode::KEY_EQUAL,
            KeyCode::BSpace => OsCode::KEY_BACKSPACE,
            KeyCode::Tab => OsCode::KEY_TAB,
            KeyCode::Q => OsCode::KEY_Q,
            KeyCode::W => OsCode::KEY_W,
            KeyCode::E => OsCode::KEY_E,
            KeyCode::R => OsCode::KEY_R,
            KeyCode::T => OsCode::KEY_T,
            KeyCode::Y => OsCode::KEY_Y,
            KeyCode::U => OsCode::KEY_U,
            KeyCode::I => OsCode::KEY_I,
            KeyCode::O => OsCode::KEY_O,
            KeyCode::P => OsCode::KEY_P,
            KeyCode::LBracket => OsCode::KEY_LEFTBRACE,
            KeyCode::RBracket => OsCode::KEY_RIGHTBRACE,
            KeyCode::Enter => OsCode::KEY_ENTER,
            KeyCode::LCtrl => OsCode::KEY_LEFTCTRL,
            KeyCode::A => OsCode::KEY_A,
            KeyCode::S => OsCode::KEY_S,
            KeyCode::D => OsCode::KEY_D,
            KeyCode::F => OsCode::KEY_F,
            KeyCode::G => OsCode::KEY_G,
            KeyCode::H => OsCode::KEY_H,
            KeyCode::J => OsCode::KEY_J,
            KeyCode::K => OsCode::KEY_K,
            KeyCode::L => OsCode::KEY_L,
            KeyCode::SColon => OsCode::KEY_SEMICOLON,
            KeyCode::Quote => OsCode::KEY_APOSTROPHE,
            KeyCode::Grave => OsCode::KEY_GRAVE,
            KeyCode::LShift => OsCode::KEY_LEFTSHIFT,
            KeyCode::Bslash => OsCode::KEY_BACKSLASH,
            KeyCode::Z => OsCode::KEY_Z,
            KeyCode::X => OsCode::KEY_X,
            KeyCode::C => OsCode::KEY_C,
            KeyCode::V => OsCode::KEY_V,
            KeyCode::B => OsCode::KEY_B,
            KeyCode::N => OsCode::KEY_N,
            KeyCode::M => OsCode::KEY_M,
            KeyCode::Comma => OsCode::KEY_COMMA,
            KeyCode::Dot => OsCode::KEY_DOT,
            KeyCode::Slash => OsCode::KEY_SLASH,
            KeyCode::RShift => OsCode::KEY_RIGHTSHIFT,
            KeyCode::KpAsterisk => OsCode::KEY_KPASTERISK,
            KeyCode::LAlt => OsCode::KEY_LEFTALT,
            KeyCode::Space => OsCode::KEY_SPACE,
            KeyCode::CapsLock => OsCode::KEY_CAPSLOCK,
            KeyCode::F1 => OsCode::KEY_F1,
            KeyCode::F2 => OsCode::KEY_F2,
            KeyCode::F3 => OsCode::KEY_F3,
            KeyCode::F4 => OsCode::KEY_F4,
            KeyCode::F5 => OsCode::KEY_F5,
            KeyCode::F6 => OsCode::KEY_F6,
            KeyCode::F7 => OsCode::KEY_F7,
            KeyCode::F8 => OsCode::KEY_F8,
            KeyCode::F9 => OsCode::KEY_F9,
            KeyCode::F10 => OsCode::KEY_F10,
            KeyCode::NumLock => OsCode::KEY_NUMLOCK,
            KeyCode::ScrollLock => OsCode::KEY_SCROLLLOCK,
            KeyCode::Kp7 => OsCode::KEY_KP7,
            KeyCode::Kp8 => OsCode::KEY_KP8,
            KeyCode::Kp9 => OsCode::KEY_KP9,
            KeyCode::KpMinus => OsCode::KEY_KPMINUS,
            KeyCode::Kp4 => OsCode::KEY_KP4,
            KeyCode::Kp5 => OsCode::KEY_KP5,
            KeyCode::Kp6 => OsCode::KEY_KP6,
            KeyCode::KpPlus => OsCode::KEY_KPPLUS,
            KeyCode::Kp1 => OsCode::KEY_KP1,
            KeyCode::Kp2 => OsCode::KEY_KP2,
            KeyCode::Kp3 => OsCode::KEY_KP3,
            KeyCode::Kp0 => OsCode::KEY_KP0,
            KeyCode::KpDot => OsCode::KEY_KPDOT,
            KeyCode::F11 => OsCode::KEY_F11,
            KeyCode::F12 => OsCode::KEY_F12,
            KeyCode::KpEnter => OsCode::KEY_KPENTER,
            KeyCode::RCtrl => OsCode::KEY_RIGHTCTRL,
            KeyCode::KpSlash => OsCode::KEY_KPSLASH,
            KeyCode::SysReq => OsCode::KEY_SYSRQ,
            KeyCode::RAlt => OsCode::KEY_RIGHTALT,
            KeyCode::Home => OsCode::KEY_HOME,
            KeyCode::Up => OsCode::KEY_UP,
            KeyCode::PgUp => OsCode::KEY_PAGEUP,
            KeyCode::Left => OsCode::KEY_LEFT,
            KeyCode::Right => OsCode::KEY_RIGHT,
            KeyCode::End => OsCode::KEY_END,
            KeyCode::Down => OsCode::KEY_DOWN,
            KeyCode::PgDown => OsCode::KEY_PAGEDOWN,
            KeyCode::Insert => OsCode::KEY_INSERT,
            KeyCode::Delete => OsCode::KEY_DELETE,
            KeyCode::Mute => OsCode::KEY_MUTE,
            KeyCode::VolDown => OsCode::KEY_VOLUMEDOWN,
            KeyCode::VolUp => OsCode::KEY_VOLUMEUP,
            KeyCode::Power => OsCode::KEY_POWER,
            KeyCode::KpEqual => OsCode::KEY_KPEQUAL,
            KeyCode::Pause => OsCode::KEY_PAUSE,
            KeyCode::KpComma => OsCode::KEY_KPCOMMA,
            KeyCode::LGui => OsCode::KEY_LEFTMETA,
            KeyCode::RGui => OsCode::KEY_RIGHTMETA,
            KeyCode::Stop => OsCode::KEY_STOP,
            KeyCode::Again => OsCode::KEY_AGAIN,
            KeyCode::Undo => OsCode::KEY_UNDO,
            KeyCode::Copy => OsCode::KEY_COPY,
            KeyCode::Paste => OsCode::KEY_PASTE,
            KeyCode::Find => OsCode::KEY_FIND,
            KeyCode::Cut => OsCode::KEY_CUT,
            KeyCode::Help => OsCode::KEY_HELP,
            KeyCode::Menu => OsCode::KEY_MENU,
            KeyCode::MediaCalc => OsCode::KEY_CALC,
            KeyCode::MediaSleep => OsCode::KEY_SLEEP,
            KeyCode::MediaWWW => OsCode::KEY_WWW,
            KeyCode::MediaCoffee => OsCode::KEY_COFFEE,
            KeyCode::MediaBack => OsCode::KEY_BACK,
            KeyCode::MediaForward => OsCode::KEY_FORWARD,
            KeyCode::MediaEjectCD => OsCode::KEY_EJECTCD,
            KeyCode::MediaNextSong => OsCode::KEY_NEXTSONG,
            KeyCode::MediaPlayPause => OsCode::KEY_PLAYPAUSE,
            KeyCode::MediaPreviousSong => OsCode::KEY_PREVIOUSSONG,
            KeyCode::MediaStopCD => OsCode::KEY_STOPCD,
            KeyCode::MediaRefresh => OsCode::KEY_REFRESH,
            KeyCode::MediaEdit => OsCode::KEY_EDIT,
            KeyCode::MediaScrollUp => OsCode::KEY_SCROLLUP,
            KeyCode::MediaScrollDown => OsCode::KEY_SCROLLDOWN,
            KeyCode::F13 => OsCode::KEY_F13,
            KeyCode::F14 => OsCode::KEY_F14,
            KeyCode::F15 => OsCode::KEY_F15,
            KeyCode::F16 => OsCode::KEY_F16,
            KeyCode::F17 => OsCode::KEY_F17,
            KeyCode::F18 => OsCode::KEY_F18,
            KeyCode::F19 => OsCode::KEY_F19,
            KeyCode::F20 => OsCode::KEY_F20,
            KeyCode::F21 => OsCode::KEY_F21,
            KeyCode::F22 => OsCode::KEY_F22,
            KeyCode::F23 => OsCode::KEY_F23,
            KeyCode::F24 => OsCode::KEY_F24,
            KeyCode::Lang1 => OsCode::KEY_HANGEUL,
            KeyCode::Lang2 => OsCode::KEY_HANJA,
            KeyCode::PScreen => OsCode::KEY_PRINT,
            KeyCode::AltErase => OsCode::KEY_ALTERASE,
            KeyCode::Cancel => OsCode::KEY_CANCEL,
            KeyCode::MediaMute => OsCode::KEY_MICMUTE,
            _ => OsCode::KEY_UNKNOWN,
        }
    }
}

impl From<&KeyCode> for OsCode {
    fn from(item: &KeyCode) -> Self {
        (*item).into()
    }
}

impl From<OsCode> for KeyCode {
    fn from(item: OsCode) -> KeyCode {
        match item {
            OsCode::KEY_ESC => KeyCode::Escape,
            OsCode::KEY_1 => KeyCode::Kb1,
            OsCode::KEY_2 => KeyCode::Kb2,
            OsCode::KEY_3 => KeyCode::Kb3,
            OsCode::KEY_4 => KeyCode::Kb4,
            OsCode::KEY_5 => KeyCode::Kb5,
            OsCode::KEY_6 => KeyCode::Kb6,
            OsCode::KEY_7 => KeyCode::Kb7,
            OsCode::KEY_8 => KeyCode::Kb8,
            OsCode::KEY_9 => KeyCode::Kb9,
            OsCode::KEY_0 => KeyCode::Kb0,
            OsCode::KEY_MINUS => KeyCode::Minus,
            OsCode::KEY_EQUAL => KeyCode::Equal,
            OsCode::KEY_BACKSPACE => KeyCode::BSpace,
            OsCode::KEY_TAB => KeyCode::Tab,
            OsCode::KEY_Q => KeyCode::Q,
            OsCode::KEY_W => KeyCode::W,
            OsCode::KEY_E => KeyCode::E,
            OsCode::KEY_R => KeyCode::R,
            OsCode::KEY_T => KeyCode::T,
            OsCode::KEY_Y => KeyCode::Y,
            OsCode::KEY_U => KeyCode::U,
            OsCode::KEY_I => KeyCode::I,
            OsCode::KEY_O => KeyCode::O,
            OsCode::KEY_P => KeyCode::P,
            OsCode::KEY_LEFTBRACE => KeyCode::LBracket,
            OsCode::KEY_RIGHTBRACE => KeyCode::RBracket,
            OsCode::KEY_ENTER => KeyCode::Enter,
            OsCode::KEY_LEFTCTRL => KeyCode::LCtrl,
            OsCode::KEY_A => KeyCode::A,
            OsCode::KEY_S => KeyCode::S,
            OsCode::KEY_D => KeyCode::D,
            OsCode::KEY_F => KeyCode::F,
            OsCode::KEY_G => KeyCode::G,
            OsCode::KEY_H => KeyCode::H,
            OsCode::KEY_J => KeyCode::J,
            OsCode::KEY_K => KeyCode::K,
            OsCode::KEY_L => KeyCode::L,
            OsCode::KEY_SEMICOLON => KeyCode::SColon,
            OsCode::KEY_APOSTROPHE => KeyCode::Quote,
            OsCode::KEY_GRAVE => KeyCode::Grave,
            OsCode::KEY_LEFTSHIFT => KeyCode::LShift,
            OsCode::KEY_BACKSLASH => KeyCode::Bslash,
            OsCode::KEY_Z => KeyCode::Z,
            OsCode::KEY_X => KeyCode::X,
            OsCode::KEY_C => KeyCode::C,
            OsCode::KEY_V => KeyCode::V,
            OsCode::KEY_B => KeyCode::B,
            OsCode::KEY_N => KeyCode::N,
            OsCode::KEY_M => KeyCode::M,
            OsCode::KEY_COMMA => KeyCode::Comma,
            OsCode::KEY_DOT => KeyCode::Dot,
            OsCode::KEY_SLASH => KeyCode::Slash,
            OsCode::KEY_RIGHTSHIFT => KeyCode::RShift,
            OsCode::KEY_KPASTERISK => KeyCode::KpAsterisk,
            OsCode::KEY_LEFTALT => KeyCode::LAlt,
            OsCode::KEY_SPACE => KeyCode::Space,
            OsCode::KEY_CAPSLOCK => KeyCode::CapsLock,
            OsCode::KEY_F1 => KeyCode::F1,
            OsCode::KEY_F2 => KeyCode::F2,
            OsCode::KEY_F3 => KeyCode::F3,
            OsCode::KEY_F4 => KeyCode::F4,
            OsCode::KEY_F5 => KeyCode::F5,
            OsCode::KEY_F6 => KeyCode::F6,
            OsCode::KEY_F7 => KeyCode::F7,
            OsCode::KEY_F8 => KeyCode::F8,
            OsCode::KEY_F9 => KeyCode::F9,
            OsCode::KEY_F10 => KeyCode::F10,
            OsCode::KEY_NUMLOCK => KeyCode::NumLock,
            OsCode::KEY_SCROLLLOCK => KeyCode::ScrollLock,
            OsCode::KEY_KP7 => KeyCode::Kp7,
            OsCode::KEY_KP8 => KeyCode::Kp8,
            OsCode::KEY_KP9 => KeyCode::Kp9,
            OsCode::KEY_KPMINUS => KeyCode::KpMinus,
            OsCode::KEY_KP4 => KeyCode::Kp4,
            OsCode::KEY_KP5 => KeyCode::Kp5,
            OsCode::KEY_KP6 => KeyCode::Kp6,
            OsCode::KEY_KPPLUS => KeyCode::KpPlus,
            OsCode::KEY_KP1 => KeyCode::Kp1,
            OsCode::KEY_KP2 => KeyCode::Kp2,
            OsCode::KEY_KP3 => KeyCode::Kp3,
            OsCode::KEY_KP0 => KeyCode::Kp0,
            OsCode::KEY_KPDOT => KeyCode::KpDot,
            OsCode::KEY_F11 => KeyCode::F11,
            OsCode::KEY_F12 => KeyCode::F12,
            OsCode::KEY_KPENTER => KeyCode::KpEnter,
            OsCode::KEY_RIGHTCTRL => KeyCode::RCtrl,
            OsCode::KEY_KPSLASH => KeyCode::KpSlash,
            OsCode::KEY_SYSRQ => KeyCode::SysReq,
            OsCode::KEY_RIGHTALT => KeyCode::RAlt,
            OsCode::KEY_HOME => KeyCode::Home,
            OsCode::KEY_UP => KeyCode::Up,
            OsCode::KEY_PAGEUP => KeyCode::PgUp,
            OsCode::KEY_LEFT => KeyCode::Left,
            OsCode::KEY_RIGHT => KeyCode::Right,
            OsCode::KEY_END => KeyCode::End,
            OsCode::KEY_DOWN => KeyCode::Down,
            OsCode::KEY_PAGEDOWN => KeyCode::PgDown,
            OsCode::KEY_INSERT => KeyCode::Insert,
            OsCode::KEY_DELETE => KeyCode::Delete,
            OsCode::KEY_MUTE => KeyCode::Mute,
            OsCode::KEY_VOLUMEDOWN => KeyCode::VolDown,
            OsCode::KEY_VOLUMEUP => KeyCode::VolUp,
            OsCode::KEY_POWER => KeyCode::Power,
            OsCode::KEY_KPEQUAL => KeyCode::KpEqual,
            OsCode::KEY_PAUSE => KeyCode::Pause,
            OsCode::KEY_KPCOMMA => KeyCode::KpComma,
            OsCode::KEY_LEFTMETA => KeyCode::LGui,
            OsCode::KEY_RIGHTMETA => KeyCode::RGui,
            OsCode::KEY_STOP => KeyCode::Stop,
            OsCode::KEY_AGAIN => KeyCode::Again,
            OsCode::KEY_UNDO => KeyCode::Undo,
            OsCode::KEY_COPY => KeyCode::Copy,
            OsCode::KEY_PASTE => KeyCode::Paste,
            OsCode::KEY_FIND => KeyCode::Find,
            OsCode::KEY_CUT => KeyCode::Cut,
            OsCode::KEY_HELP => KeyCode::Help,
            OsCode::KEY_MENU => KeyCode::Menu,
            OsCode::KEY_CALC => KeyCode::MediaCalc,
            OsCode::KEY_SLEEP => KeyCode::MediaSleep,
            OsCode::KEY_WWW => KeyCode::MediaWWW,
            OsCode::KEY_COFFEE => KeyCode::MediaCoffee,
            OsCode::KEY_BACK => KeyCode::MediaBack,
            OsCode::KEY_FORWARD => KeyCode::MediaForward,
            OsCode::KEY_EJECTCD => KeyCode::MediaEjectCD,
            OsCode::KEY_NEXTSONG => KeyCode::MediaNextSong,
            OsCode::KEY_PLAYPAUSE => KeyCode::MediaPlayPause,
            OsCode::KEY_PREVIOUSSONG => KeyCode::MediaPreviousSong,
            OsCode::KEY_STOPCD => KeyCode::MediaStopCD,
            OsCode::KEY_REFRESH => KeyCode::MediaRefresh,
            OsCode::KEY_EDIT => KeyCode::MediaEdit,
            OsCode::KEY_SCROLLUP => KeyCode::MediaScrollUp,
            OsCode::KEY_SCROLLDOWN => KeyCode::MediaScrollDown,
            OsCode::KEY_F13 => KeyCode::F13,
            OsCode::KEY_F14 => KeyCode::F14,
            OsCode::KEY_F15 => KeyCode::F15,
            OsCode::KEY_F16 => KeyCode::F16,
            OsCode::KEY_F17 => KeyCode::F17,
            OsCode::KEY_F18 => KeyCode::F18,
            OsCode::KEY_F19 => KeyCode::F19,
            OsCode::KEY_F20 => KeyCode::F20,
            OsCode::KEY_F21 => KeyCode::F21,
            OsCode::KEY_F22 => KeyCode::F22,
            OsCode::KEY_F23 => KeyCode::F23,
            OsCode::KEY_F24 => KeyCode::F24,
            OsCode::KEY_HANGEUL => KeyCode::Lang1,
            OsCode::KEY_HANJA => KeyCode::Lang2,
            OsCode::KEY_PRINT => KeyCode::PScreen,
            OsCode::KEY_ALTERASE => KeyCode::AltErase,
            OsCode::KEY_CANCEL => KeyCode::Cancel,
            OsCode::KEY_MICMUTE => KeyCode::MediaMute,
            _ => KeyCode::No,
        }
    }
}

impl From<&OsCode> for KeyCode {
    fn from(item: &OsCode) -> KeyCode {
        (*item).into()
    }
}

// ------------------ KeyValue --------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyValue {
    Release = 0,
    Press = 1,
    Repeat = 2,
}

impl From<i32> for KeyValue {
    fn from(item: i32) -> Self {
        match item {
            0 => Self::Release,
            1 => Self::Press,
            2 => Self::Repeat,
            _ => unreachable!(),
        }
    }
}

impl From<bool> for KeyValue {
    fn from(up: bool) -> Self {
        match up {
            true => Self::Release,
            false => Self::Press,
        }
    }
}

impl From<KeyValue> for bool {
    fn from(val: KeyValue) -> Self {
        matches!(val, KeyValue::Release)
    }
}

#[derive(Debug)]
pub struct KeyEvent {
    pub code: OsCode,
    pub value: KeyValue,
}

impl KeyEvent {
    pub fn new(code: OsCode, value: KeyValue) -> Self {
        Self { code, value }
    }
}

impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();
    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        Ok(Self {
            code: OsCode::from_u32(item.code as u32).ok_or(())?,
            value: match item.up {
                true => KeyValue::Release,
                false => KeyValue::Press,
            },
        })
    }
}

impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        Self {
            code: item.code.into(),
            up: item.value.into(),
            time: 0,
        }
    }
}
