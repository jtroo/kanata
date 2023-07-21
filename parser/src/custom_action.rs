//! This module contains the "Custom" actions that are used with the keyberon layout.
//!
//! When adding a new custom action, the macro section of the config.adoc documentation may need to
//! be updated, to include the new action to the documented list of supported actions in macro.

use anyhow::{anyhow, Result};
use kanata_keyberon::key_code::KeyCode;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CustomAction {
    Cmd(Vec<String>),
    CmdOutputKeys(Vec<String>),
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
    Delay(u16),
    DelayOnRelease(u16),
    MWheel {
        direction: MWheelDirection,
        interval: u16,
        distance: u16,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MWheelDirection {
    Up,
    Down,
    Left,
    Right,
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
