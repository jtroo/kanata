//! Contains all list action names and a function to check that an action name is that of a list
//! action.

// Note: changing any of these constants is a breaking change.
pub const LAYER_SWITCH: &str = "layer-switch";
pub const LAYER_TOGGLE: &str = "layer-toggle";
pub const LAYER_WHILE_HELD: &str = "layer-while-held";
pub const TAP_HOLD: &str = "tap-hold";
pub const TAP_HOLD_PRESS: &str = "tap-hold-press";
pub const TAP_HOLD_RELEASE: &str = "tap-hold-release";
pub const TAP_HOLD_PRESS_TIMEOUT: &str = "tap-hold-press-timeout";
pub const TAP_HOLD_RELEASE_TIMEOUT: &str = "tap-hold-release-timeout";
pub const TAP_HOLD_RELEASE_KEYS: &str = "tap-hold-release-keys";
pub const TAP_HOLD_EXCEPT_KEYS: &str = "tap-hold-except-keys";
pub const MULTI: &str = "multi";
pub const MACRO: &str = "macro";
pub const MACRO_REPEAT: &str = "macro-repeat";
pub const MACRO_RELEASE_CANCEL: &str = "macro-release-cancel";
pub const MACRO_REPEAT_RELEASE_CANCEL: &str = "macro-repeat-release-cancel";
pub const UNICODE: &str = "unicode";
pub const ONE_SHOT: &str = "one-shot";
pub const ONE_SHOT_PRESS: &str = "one-shot-press";
pub const ONE_SHOT_RELEASE: &str = "one-shot-release";
pub const ONE_SHOT_PRESS_PCANCEL: &str = "one-shot-press-pcancel";
pub const ONE_SHOT_RELEASE_PCANCEL: &str = "one-shot-release-pcancel";
pub const TAP_DANCE: &str = "tap-dance";
pub const TAP_DANCE_EAGER: &str = "tap-dance-eager";
pub const CHORD: &str = "chord";
pub const RELEASE_KEY: &str = "release-key";
pub const RELEASE_LAYER: &str = "release-layer";
pub const ON_PRESS_FAKEKEY: &str = "on-press-fakekey";
pub const ON_RELEASE_FAKEKEY: &str = "on-release-fakekey";
pub const ON_PRESS_FAKEKEY_DELAY: &str = "on-press-fakekey-delay";
pub const ON_RELEASE_FAKEKEY_DELAY: &str = "on-release-fakekey-delay";
pub const ON_IDLE_FAKEKEY: &str = "on-idle-fakekey";
pub const MWHEEL_UP: &str = "mwheel-up";
pub const MWHEEL_DOWN: &str = "mwheel-down";
pub const MWHEEL_LEFT: &str = "mwheel-left";
pub const MWHEEL_RIGHT: &str = "mwheel-right";
pub const MOVEMOUSE_UP: &str = "movemouse-up";
pub const MOVEMOUSE_DOWN: &str = "movemouse-down";
pub const MOVEMOUSE_LEFT: &str = "movemouse-left";
pub const MOVEMOUSE_RIGHT: &str = "movemouse-right";
pub const MOVEMOUSE_ACCEL_UP: &str = "movemouse-accel-up";
pub const MOVEMOUSE_ACCEL_DOWN: &str = "movemouse-accel-down";
pub const MOVEMOUSE_ACCEL_LEFT: &str = "movemouse-accel-left";
pub const MOVEMOUSE_ACCEL_RIGHT: &str = "movemouse-accel-right";
pub const MOVEMOUSE_SPEED: &str = "movemouse-speed";
pub const SETMOUSE: &str = "setmouse";
pub const DYNAMIC_MACRO_RECORD: &str = "dynamic-macro-record";
pub const DYNAMIC_MACRO_PLAY: &str = "dynamic-macro-play";
pub const ARBITRARY_CODE: &str = "arbitrary-code";
pub const CMD: &str = "cmd";
pub const CMD_OUTPUT_KEYS: &str = "cmd-output-keys";
pub const FORK: &str = "fork";
pub const CAPS_WORD: &str = "caps-word";
pub const CAPS_WORD_CUSTOM: &str = "caps-word-custom";
pub const DYNAMIC_MACRO_RECORD_STOP_TRUNCATE: &str = "dynamic-macro-record-stop-truncate";
pub const SWITCH: &str = "switch";
pub const SEQUENCE: &str = "sequence";
pub const UNMOD: &str = "unmod";
pub const UNSHIFT: &str = "unshift";

pub fn is_list_action(ac: &str) -> bool {
    const LIST_ACTIONS: [&str; 58] = [
        LAYER_SWITCH,
        LAYER_TOGGLE,
        LAYER_WHILE_HELD,
        TAP_HOLD,
        TAP_HOLD_PRESS,
        TAP_HOLD_RELEASE,
        TAP_HOLD_PRESS_TIMEOUT,
        TAP_HOLD_RELEASE_TIMEOUT,
        TAP_HOLD_RELEASE_KEYS,
        TAP_HOLD_EXCEPT_KEYS,
        MULTI,
        MACRO,
        MACRO_REPEAT,
        MACRO_RELEASE_CANCEL,
        MACRO_REPEAT_RELEASE_CANCEL,
        UNICODE,
        ONE_SHOT,
        ONE_SHOT_PRESS,
        ONE_SHOT_RELEASE,
        ONE_SHOT_PRESS_PCANCEL,
        ONE_SHOT_RELEASE_PCANCEL,
        TAP_DANCE,
        TAP_DANCE_EAGER,
        CHORD,
        RELEASE_KEY,
        RELEASE_LAYER,
        ON_PRESS_FAKEKEY,
        ON_RELEASE_FAKEKEY,
        ON_PRESS_FAKEKEY_DELAY,
        ON_RELEASE_FAKEKEY_DELAY,
        ON_IDLE_FAKEKEY,
        MWHEEL_UP,
        MWHEEL_DOWN,
        MWHEEL_LEFT,
        MWHEEL_RIGHT,
        MOVEMOUSE_UP,
        MOVEMOUSE_DOWN,
        MOVEMOUSE_LEFT,
        MOVEMOUSE_RIGHT,
        MOVEMOUSE_ACCEL_UP,
        MOVEMOUSE_ACCEL_DOWN,
        MOVEMOUSE_ACCEL_LEFT,
        MOVEMOUSE_ACCEL_RIGHT,
        MOVEMOUSE_SPEED,
        SETMOUSE,
        DYNAMIC_MACRO_RECORD,
        DYNAMIC_MACRO_PLAY,
        ARBITRARY_CODE,
        CMD,
        CMD_OUTPUT_KEYS,
        FORK,
        CAPS_WORD,
        CAPS_WORD_CUSTOM,
        DYNAMIC_MACRO_RECORD_STOP_TRUNCATE,
        SWITCH,
        SEQUENCE,
        UNMOD,
        UNSHIFT,
    ];
    LIST_ACTIONS.contains(&ac)
}
