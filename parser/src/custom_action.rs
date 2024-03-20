//! This module contains the "Custom" actions that are used with the keyberon layout.
//!
//! When adding a new custom action, the macro section of the config.adoc documentation may need to
//! be updated, to include the new action to the documented list of supported actions in macro.

use anyhow::{anyhow, Result};
use kanata_keyberon::key_code::KeyCode;

use crate::{cfg::SimpleSExpr, keys::OsCode};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CustomAction {
    Cmd(Vec<String>),
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
        keys: Vec<KeyCode>,
    },
    Unshifted {
        keys: Vec<KeyCode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Btn {
    Left,
    Right,
    Mid,
    Forward,
    Backward,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MWheelDirection {
    Up,
    Down,
    Left,
    Right,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapsWordCfg {
    pub keys_to_capitalize: &'static [KeyCode],
    pub keys_nonterminal: &'static [KeyCode],
    pub timeout: u16,
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
        format!("sequence input mode must be one of: {SEQ_VISIBLE_BACKSPACED}, {SEQ_HIDDEN_SUPPRESSED}, {SEQ_HIDDEN_DELAY_TYPE}")
    }
}
