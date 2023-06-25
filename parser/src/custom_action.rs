//! This module contains the "Custom" actions that are used with the keyberon layout.
//!
//! When adding a new custom action, the macro section of the config.adoc documentation may need to
//! be updated, to include the new action to the documented list of supported actions in macro.

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
    SequenceLeader,
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
