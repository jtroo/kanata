//! This module contains the "Custom" actions that are used with the keyberon layout.
//!
//! When adding a new custom action, the macro section of the config.adoc documentation may need to
//! be updated, to include the new action to the documented list of supported actions in macro.

use anyhow::{Result, anyhow};
use core::fmt;
use kanata_keyberon::key_code::KeyCode;

use crate::{cfg::SimpleSExpr, keys::OsCode};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CustomAction {
    Cmd(Vec<String>),
    CmdLog(LogLevel, LogLevel, Vec<String>),
    CmdOutputKeys(Vec<String>),
    PushMessage(Vec<SimpleSExpr>),
    Unicode(char),
    Mouse(Btn),
    MouseTap(Btn),
    FakeKey {
        coord: Coord,
        action: FakeKeyAction,
    },
    FakeKeyOnRelease {
        coord: Coord,
        action: FakeKeyAction,
    },
    FakeKeyOnIdle(FakeKeyOnIdle),
    FakeKeyOnPhysicalIdle(FakeKeyOnIdle),
    FakeKeyHoldForDuration(FakeKeyHoldForDuration),
    Delay(u16),
    DelayOnRelease(u16),
    MWheel {
        direction: MWheelDirection,
        interval: u16,
        distance: u16,
    },
    MWheelNotch {
        direction: MWheelDirection,
    },
    MoveMouse {
        direction: MoveDirection,
        interval: u16,
        distance: u16,
    },
    MoveMouseAccel {
        direction: MoveDirection,
        interval: u16,
        accel_time: u16,
        min_distance: u16,
        max_distance: u16,
    },
    MoveMouseSpeed {
        speed: u16,
    },
    SequenceCancel,
    SequenceLeader(u16, SequenceInputMode),
    /// Purpose:
    /// In case the user has dead keys in their OS layout, they may wish to send fewer backspaces upon
    /// a successful completion of visible-backspaced sequences, because the number of key events
    /// is larger than the number of backspace-able symbols typed within the application.
    /// This custom action is a marker to accomplish the use case.
    SequenceNoerase(u16),
    LiveReload,
    LiveReloadNext,
    LiveReloadPrev,
    /// Live-reload the n'th configuration file provided on the CLI. This should begin with 0 as
    /// the first configuration file provided. The rest of the parser code is free to choose 0 or 1
    /// as the user-facing value though.
    LiveReloadNum(u16),
    LiveReloadFile(String),
    Repeat,
    CancelMacroOnRelease,
    CancelMacroOnNextPress(u32),
    DynamicMacroRecord(u16),
    DynamicMacroRecordStop(u16),
    DynamicMacroPlay(u16),
    SendArbitraryCode(u16),
    CapsWord(CapsWordCfg),
    SetMouse {
        x: u16,
        y: u16,
    },
    Unmodded {
        keys: Box<[KeyCode]>,
        mods: UnmodMods,
    },
    Unshifted {
        keys: Box<[KeyCode]>,
    },
    ReverseReleaseOrder,
    ClipboardSet(String),
    ClipboardCmdSet(Vec<String>),
    ClipboardSave(u16),
    ClipboardRestore(u16),
    ClipboardSaveSet(u16, String),
    ClipboardSaveCmdSet(u16, Vec<String>),
    ClipboardSaveSwap(u16, u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Btn {
    Left,
    Right,
    Mid,
    Forward,
    Backward,
}

impl fmt::Display for Btn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Btn::Left => write!(f, "‹🖰"),
            Btn::Right => write!(f, "🖰›"),
            Btn::Mid => write!(f, "🖱"),
            Btn::Backward => write!(f, "⎌🖰"),
            Btn::Forward => write!(f, "🖰↷"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord {
    pub x: u8,
    pub y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FakeKeyAction {
    Press,
    Release,
    Tap,
    Toggle,
}

/// An active waiting-for-idle state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FakeKeyOnIdle {
    pub coord: Coord,
    pub action: FakeKeyAction,
    pub idle_duration: u16,
}

/// Information for an action that presses a fake key / vkey that becomes released on a
/// renewable-when-reactivated deadline.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FakeKeyHoldForDuration {
    pub coord: Coord,
    pub hold_duration: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MWheelDirection {
    Up,
    Down,
    Left,
    Right,
}
impl fmt::Display for MWheelDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MWheelDirection::Up => write!(f, "🖱↑"),
            MWheelDirection::Down => write!(f, "🖱↓"),
            MWheelDirection::Left => write!(f, "🖱←"),
            MWheelDirection::Right => write!(f, "🖱→"),
        }
    }
}

impl TryFrom<OsCode> for MWheelDirection {
    type Error = ();
    fn try_from(value: OsCode) -> Result<Self, Self::Error> {
        use OsCode::*;
        Ok(match value {
            MouseWheelUp => MWheelDirection::Up,
            MouseWheelDown => MWheelDirection::Down,
            MouseWheelLeft => MWheelDirection::Left,
            MouseWheelRight => MWheelDirection::Right,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
}
impl fmt::Display for MoveDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MoveDirection::Up => write!(f, "↑"),
            MoveDirection::Down => write!(f, "↓"),
            MoveDirection::Left => write!(f, "←"),
            MoveDirection::Right => write!(f, "→"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapsWordCfg {
    pub keys_to_capitalize: &'static [KeyCode],
    pub keys_nonterminal: &'static [KeyCode],
    pub timeout: u16,
    pub repress_behaviour: CapsWordRepressBehaviour,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapsWordRepressBehaviour {
    Overwrite,
    Toggle,
}

/// This controls the behaviour of kanata when sequence mode is initiated by the sequence leader
/// action.
///
/// - `HiddenSuppressed` hides the keys typed as part of the sequence and does not output the keys
///   typed when an invalid sequence is the result of an invalid sequence character or a timeout.
/// - `HiddenDelayType` hides the keys typed as part of the sequence and outputs the keys when an
///   typed when an invalid sequence is the result of an invalid sequence character or a timeout.
/// - `VisibleBackspaced` will type the keys that are typed as part of the sequence but will
///   backspace the typed sequence keys before performing the fake key tap when a valid sequence is
///   the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequenceInputMode {
    HiddenSuppressed,
    HiddenDelayType,
    VisibleBackspaced,
}

const SEQ_VISIBLE_BACKSPACED: &str = "visible-backspaced";
const SEQ_HIDDEN_SUPPRESSED: &str = "hidden-suppressed";
const SEQ_HIDDEN_DELAY_TYPE: &str = "hidden-delay-type";

impl SequenceInputMode {
    pub fn try_from_str(s: &str) -> Result<Self> {
        match s {
            SEQ_VISIBLE_BACKSPACED => Ok(SequenceInputMode::VisibleBackspaced),
            SEQ_HIDDEN_SUPPRESSED => Ok(SequenceInputMode::HiddenSuppressed),
            SEQ_HIDDEN_DELAY_TYPE => Ok(SequenceInputMode::HiddenDelayType),
            _ => Err(anyhow!(SequenceInputMode::err_msg())),
        }
    }

    pub fn err_msg() -> String {
        format!(
            "sequence input mode must be one of: {SEQ_VISIBLE_BACKSPACED}, {SEQ_HIDDEN_SUPPRESSED}, {SEQ_HIDDEN_DELAY_TYPE}"
        )
    }
}

const LOG_LEVEL_DEBUG: &str = "debug";
const LOG_LEVEL_INFO: &str = "info";
const LOG_LEVEL_WARN: &str = "warn";
const LOG_LEVEL_ERROR: &str = "error";
const LOG_LEVEL_NONE: &str = "none";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    // No trace here because that wouldn't make sense
    Debug,
    Info,
    Warn,
    Error,
    None,
}

impl LogLevel {
    pub fn try_from_str(s: &str) -> Result<Self> {
        match s {
            LOG_LEVEL_DEBUG => Ok(LogLevel::Debug),
            LOG_LEVEL_INFO => Ok(LogLevel::Info),
            LOG_LEVEL_WARN => Ok(LogLevel::Warn),
            LOG_LEVEL_ERROR => Ok(LogLevel::Error),
            LOG_LEVEL_NONE => Ok(LogLevel::None),
            _ => Err(anyhow!(LogLevel::err_msg())),
        }
    }

    pub fn get_level(&self) -> Option<log::Level> {
        match self {
            LogLevel::Debug => Some(log::Level::Debug),
            LogLevel::Info => Some(log::Level::Info),
            LogLevel::Warn => Some(log::Level::Warn),
            LogLevel::Error => Some(log::Level::Error),
            LogLevel::None => None,
        }
    }

    pub fn err_msg() -> String {
        format!(
            "log level must be one of: {LOG_LEVEL_DEBUG}, {LOG_LEVEL_INFO}, {LOG_LEVEL_WARN}, {LOG_LEVEL_ERROR}, {LOG_LEVEL_NONE}"
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnmodMods(u8);

bitflags::bitflags! {
    impl UnmodMods: u8 {
        const LSft = 0b00000001;
        const RSft = 0b00000010;
        const LAlt = 0b00000100;
        const RAlt = 0b00001000;
        const LCtl = 0b00010000;
        const RCtl = 0b00100000;
        const LMet = 0b01000000;
        const RMet = 0b10000000;
    }
}
