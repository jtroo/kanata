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
    SequenceLeader,
    LiveReload,
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Btn {
    Left,
    Right,
    Mid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord {
    pub x: u8,
    pub y: u8,
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
