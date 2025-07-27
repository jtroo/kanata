//! Contains all list action names and a function to check that an action name is that of a list
//! action.

// Note: changing any of these constants is a breaking change.
pub const LAYER_SWITCH: &str = "layer-switch";
pub const LAYER_TOGGLE: &str = "layer-toggle";
pub const LAYER_WHILE_HELD: &str = "layer-while-held";
pub const TAP_HOLD: &str = "tap-hold";
pub const TAP_HOLD_PRESS: &str = "tap-hold-press";
pub const TAP_HOLD_PRESS_A: &str = "tap⬓↓";
pub const TAP_HOLD_RELEASE: &str = "tap-hold-release";
pub const TAP_HOLD_RELEASE_A: &str = "tap⬓↑";
pub const TAP_HOLD_PRESS_TIMEOUT: &str = "tap-hold-press-timeout";
pub const TAP_HOLD_PRESS_TIMEOUT_A: &str = "tap⬓↓timeout";
pub const TAP_HOLD_RELEASE_TIMEOUT: &str = "tap-hold-release-timeout";
pub const TAP_HOLD_RELEASE_TIMEOUT_A: &str = "tap⬓↑timeout";
pub const TAP_HOLD_RELEASE_KEYS: &str = "tap-hold-release-keys";
pub const TAP_HOLD_RELEASE_KEYS_A: &str = "tap⬓↑keys";
pub const TAP_HOLD_EXCEPT_KEYS: &str = "tap-hold-except-keys";
pub const TAP_HOLD_EXCEPT_KEYS_A: &str = "tap⬓⤫keys";
pub const MULTI: &str = "multi";
pub const MACRO: &str = "macro";
pub const MACRO_REPEAT: &str = "macro-repeat";
pub const MACRO_REPEAT_A: &str = "macro⟳";
pub const MACRO_RELEASE_CANCEL: &str = "macro-release-cancel";
pub const MACRO_RELEASE_CANCEL_A: &str = "macro↑⤫";
pub const MACRO_REPEAT_RELEASE_CANCEL: &str = "macro-repeat-release-cancel";
pub const MACRO_REPEAT_RELEASE_CANCEL_A: &str = "macro⟳↑⤫";
pub const MACRO_CANCEL_ON_NEXT_PRESS: &str = "macro-cancel-on-press";
pub const MACRO_REPEAT_CANCEL_ON_NEXT_PRESS: &str = "macro-repeat-cancel-on-press";
pub const MACRO_CANCEL_ON_NEXT_PRESS_CANCEL_ON_RELEASE: &str =
    "macro-release-cancel-and-cancel-on-press";
pub const MACRO_REPEAT_CANCEL_ON_NEXT_PRESS_CANCEL_ON_RELEASE: &str =
    "macro-repeat-release-cancel-and-cancel-on-press";
pub const UNICODE: &str = "unicode";
pub const SYM: &str = "🔣";
pub const ONE_SHOT: &str = "one-shot";
pub const ONE_SHOT_PRESS: &str = "one-shot-press";
pub const ONE_SHOT_PRESS_A: &str = "one-shot↓";
pub const ONE_SHOT_RELEASE: &str = "one-shot-release";
pub const ONE_SHOT_RELEASE_A: &str = "one-shot↑";
pub const ONE_SHOT_PRESS_PCANCEL: &str = "one-shot-press-pcancel";
pub const ONE_SHOT_PRESS_PCANCEL_A: &str = "one-shot↓⤫";
pub const ONE_SHOT_RELEASE_PCANCEL: &str = "one-shot-release-pcancel";
pub const ONE_SHOT_RELEASE_PCANCEL_A: &str = "one-shot↑⤫";
pub const ONE_SHOT_PAUSE_PROCESSING: &str = "one-shot-pause-processing";
pub const TAP_DANCE: &str = "tap-dance";
pub const TAP_DANCE_EAGER: &str = "tap-dance-eager";
pub const CHORD: &str = "chord";
pub const RELEASE_KEY: &str = "release-key";
pub const RELEASE_KEY_A: &str = "key↑";
pub const RELEASE_LAYER: &str = "release-layer";
pub const RELEASE_LAYER_A: &str = "layer↑";
pub const ON_PRESS_FAKEKEY: &str = "on-press-fakekey";
pub const ON_PRESS_FAKEKEY_A: &str = "on↓fakekey";
pub const ON_RELEASE_FAKEKEY: &str = "on-release-fakekey";
pub const ON_RELEASE_FAKEKEY_A: &str = "on↑fakekey";
pub const ON_PRESS_DELAY: &str = "on-press-delay";
pub const ON_RELEASE_DELAY: &str = "on-release-delay";
pub const ON_PRESS_FAKEKEY_DELAY: &str = "on-press-fakekey-delay";
pub const ON_PRESS_FAKEKEY_DELAY_A: &str = "on↓fakekey-delay";
pub const ON_RELEASE_FAKEKEY_DELAY: &str = "on-release-fakekey-delay";
pub const ON_RELEASE_FAKEKEY_DELAY_A: &str = "on↑fakekey-delay";
pub const ON_IDLE_FAKEKEY: &str = "on-idle-fakekey";
pub const MWHEEL_UP: &str = "mwheel-up";
pub const MWHEEL_DOWN: &str = "mwheel-down";
pub const MWHEEL_LEFT: &str = "mwheel-left";
pub const MWHEEL_RIGHT: &str = "mwheel-right";
pub const MWHEEL_UP_A: &str = "🖱☸↑";
pub const MWHEEL_DOWN_A: &str = "🖱☸↓";
pub const MWHEEL_LEFT_A: &str = "🖱☸←";
pub const MWHEEL_RIGHT_A: &str = "🖱☸→";
pub const MOVEMOUSE_UP: &str = "movemouse-up";
pub const MOVEMOUSE_DOWN: &str = "movemouse-down";
pub const MOVEMOUSE_LEFT: &str = "movemouse-left";
pub const MOVEMOUSE_RIGHT: &str = "movemouse-right";
pub const MOVEMOUSE_ACCEL_UP: &str = "movemouse-accel-up";
pub const MOVEMOUSE_ACCEL_DOWN: &str = "movemouse-accel-down";
pub const MOVEMOUSE_ACCEL_LEFT: &str = "movemouse-accel-left";
pub const MOVEMOUSE_ACCEL_RIGHT: &str = "movemouse-accel-right";
pub const MOVEMOUSE_SPEED: &str = "movemouse-speed";
pub const MOVEMOUSE_UP_A: &str = "🖱↑";
pub const MOVEMOUSE_DOWN_A: &str = "🖱↓";
pub const MOVEMOUSE_LEFT_A: &str = "🖱←";
pub const MOVEMOUSE_RIGHT_A: &str = "🖱→";
pub const MOVEMOUSE_ACCEL_UP_A: &str = "🖱accel↑";
pub const MOVEMOUSE_ACCEL_DOWN_A: &str = "🖱accel↓";
pub const MOVEMOUSE_ACCEL_LEFT_A: &str = "🖱accel←";
pub const MOVEMOUSE_ACCEL_RIGHT_A: &str = "🖱accel→";
pub const MOVEMOUSE_SPEED_A: &str = "🖱speed";
pub const SETMOUSE: &str = "setmouse";
pub const SETMOUSE_A: &str = "set🖱";
pub const DYNAMIC_MACRO_RECORD: &str = "dynamic-macro-record";
pub const DYNAMIC_MACRO_PLAY: &str = "dynamic-macro-play";
pub const ARBITRARY_CODE: &str = "arbitrary-code";
pub const CMD: &str = "cmd";
pub const CMD_LOG: &str = "cmd-log";
pub const PUSH_MESSAGE: &str = "push-msg";
pub const CMD_OUTPUT_KEYS: &str = "cmd-output-keys";
pub const FORK: &str = "fork";
pub const CAPS_WORD: &str = "caps-word";
pub const CAPS_WORD_A: &str = "word⇪";
pub const CAPS_WORD_CUSTOM: &str = "caps-word-custom";
pub const CAPS_WORD_CUSTOM_A: &str = "word⇪custom";
pub const CAPS_WORD_TOGGLE: &str = "caps-word-toggle";
pub const CAPS_WORD_TOGGLE_A: &str = "word⇪toggle";
pub const CAPS_WORD_CUSTOM_TOGGLE: &str = "caps-word-custom-toggle";
pub const CAPS_WORD_CUSTOM_TOGGLE_A: &str = "word⇪custom-toggle";
pub const DYNAMIC_MACRO_RECORD_STOP_TRUNCATE: &str = "dynamic-macro-record-stop-truncate";
pub const SWITCH: &str = "switch";
pub const SEQUENCE: &str = "sequence";
pub const SEQUENCE_NOERASE: &str = "sequence-noerase";
pub const UNMOD: &str = "unmod";
pub const UNSHIFT: &str = "unshift";
pub const UNSHIFT_A: &str = "un⇧";
pub const LIVE_RELOAD_NUM: &str = "lrld-num";
pub const LIVE_RELOAD_FILE: &str = "lrld-file";
pub const ON_PRESS: &str = "on-press";
pub const ON_PRESS_A: &str = "on↓";
pub const ON_RELEASE: &str = "on-release";
pub const ON_RELEASE_A: &str = "on↑";
pub const ON_IDLE: &str = "on-idle";
pub const ON_PHYSICAL_IDLE: &str = "on-physical-idle";
pub const HOLD_FOR_DURATION: &str = "hold-for-duration";
pub const CLIPBOARD_SET: &str = "clipboard-set";
pub const CLIPBOARD_CMD_SET: &str = "clipboard-cmd-set";
pub const CLIPBOARD_SAVE: &str = "clipboard-save";
pub const CLIPBOARD_RESTORE: &str = "clipboard-restore";
pub const CLIPBOARD_SAVE_SET: &str = "clipboard-save-set";
pub const CLIPBOARD_SAVE_CMD_SET: &str = "clipboard-save-cmd-set";
pub const CLIPBOARD_SAVE_SWAP: &str = "clipboard-save-swap";

pub fn is_list_action(ac: &str) -> bool {
    const LIST_ACTIONS: &[&str] = &[
        LAYER_SWITCH,
        LAYER_TOGGLE,
        LAYER_WHILE_HELD,
        TAP_HOLD,
        TAP_HOLD_PRESS,
        TAP_HOLD_PRESS_A,
        TAP_HOLD_RELEASE,
        TAP_HOLD_RELEASE_A,
        TAP_HOLD_PRESS_TIMEOUT,
        TAP_HOLD_PRESS_TIMEOUT_A,
        TAP_HOLD_RELEASE_TIMEOUT,
        TAP_HOLD_RELEASE_TIMEOUT_A,
        TAP_HOLD_RELEASE_KEYS,
        TAP_HOLD_RELEASE_KEYS_A,
        TAP_HOLD_EXCEPT_KEYS,
        TAP_HOLD_EXCEPT_KEYS_A,
        MULTI,
        MACRO,
        MACRO_REPEAT,
        MACRO_REPEAT_A,
        MACRO_RELEASE_CANCEL,
        MACRO_RELEASE_CANCEL_A,
        MACRO_REPEAT_RELEASE_CANCEL,
        MACRO_REPEAT_RELEASE_CANCEL_A,
        UNICODE,
        SYM,
        ONE_SHOT,
        ONE_SHOT_PRESS,
        ONE_SHOT_PRESS_A,
        ONE_SHOT_RELEASE,
        ONE_SHOT_RELEASE_A,
        ONE_SHOT_PRESS_PCANCEL,
        ONE_SHOT_PRESS_PCANCEL_A,
        ONE_SHOT_RELEASE_PCANCEL,
        ONE_SHOT_RELEASE_PCANCEL_A,
        TAP_DANCE,
        TAP_DANCE_EAGER,
        CHORD,
        RELEASE_KEY,
        RELEASE_KEY_A,
        RELEASE_LAYER,
        RELEASE_LAYER_A,
        ON_PRESS_FAKEKEY,
        ON_PRESS_FAKEKEY_A,
        ON_RELEASE_FAKEKEY,
        ON_RELEASE_FAKEKEY_A,
        ON_PRESS_DELAY,
        ON_RELEASE_DELAY,
        ON_PRESS_FAKEKEY_DELAY,
        ON_PRESS_FAKEKEY_DELAY_A,
        ON_RELEASE_FAKEKEY_DELAY,
        ON_RELEASE_FAKEKEY_DELAY_A,
        ON_IDLE_FAKEKEY,
        MWHEEL_UP,
        MWHEEL_UP_A,
        MWHEEL_DOWN,
        MWHEEL_DOWN_A,
        MWHEEL_LEFT,
        MWHEEL_LEFT_A,
        MWHEEL_RIGHT,
        MWHEEL_RIGHT_A,
        MOVEMOUSE_UP,
        MOVEMOUSE_UP_A,
        MOVEMOUSE_DOWN,
        MOVEMOUSE_DOWN_A,
        MOVEMOUSE_LEFT,
        MOVEMOUSE_LEFT_A,
        MOVEMOUSE_RIGHT,
        MOVEMOUSE_RIGHT_A,
        MOVEMOUSE_ACCEL_UP,
        MOVEMOUSE_ACCEL_UP_A,
        MOVEMOUSE_ACCEL_DOWN,
        MOVEMOUSE_ACCEL_DOWN_A,
        MOVEMOUSE_ACCEL_LEFT,
        MOVEMOUSE_ACCEL_LEFT_A,
        MOVEMOUSE_ACCEL_RIGHT,
        MOVEMOUSE_ACCEL_RIGHT_A,
        MOVEMOUSE_SPEED,
        MOVEMOUSE_SPEED_A,
        SETMOUSE,
        SETMOUSE_A,
        DYNAMIC_MACRO_RECORD,
        DYNAMIC_MACRO_PLAY,
        ARBITRARY_CODE,
        CMD,
        CMD_OUTPUT_KEYS,
        CMD_LOG,
        PUSH_MESSAGE,
        FORK,
        CAPS_WORD,
        CAPS_WORD_A,
        CAPS_WORD_TOGGLE,
        CAPS_WORD_TOGGLE_A,
        CAPS_WORD_CUSTOM,
        CAPS_WORD_CUSTOM_A,
        CAPS_WORD_CUSTOM_TOGGLE,
        CAPS_WORD_CUSTOM_TOGGLE_A,
        DYNAMIC_MACRO_RECORD_STOP_TRUNCATE,
        SWITCH,
        SEQUENCE,
        SEQUENCE_NOERASE,
        UNMOD,
        UNSHIFT,
        UNSHIFT_A,
        LIVE_RELOAD_NUM,
        LIVE_RELOAD_FILE,
        ON_PRESS,
        ON_PRESS_A,
        ON_RELEASE,
        ON_RELEASE_A,
        ON_IDLE,
        ON_PHYSICAL_IDLE,
        HOLD_FOR_DURATION,
        MACRO_CANCEL_ON_NEXT_PRESS,
        MACRO_REPEAT_CANCEL_ON_NEXT_PRESS,
        MACRO_CANCEL_ON_NEXT_PRESS_CANCEL_ON_RELEASE,
        MACRO_REPEAT_CANCEL_ON_NEXT_PRESS_CANCEL_ON_RELEASE,
        ONE_SHOT_PAUSE_PROCESSING,
        CLIPBOARD_SET,
        CLIPBOARD_CMD_SET,
        CLIPBOARD_SAVE,
        CLIPBOARD_RESTORE,
        CLIPBOARD_SAVE_SET,
        CLIPBOARD_SAVE_CMD_SET,
        CLIPBOARD_SAVE_SWAP,
    ];
    LIST_ACTIONS.contains(&ac)
}
