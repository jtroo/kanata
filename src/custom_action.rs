#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomAction {
    Cmd(&'static [String]),
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
    Repeat,
    CancelMacroOnRelease,
    DynamicMacroRecord(u16),
    DynamicMacroRecordStop,
    DynamicMacroPlay(u16),
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
