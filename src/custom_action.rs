#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomAction {
    Cmd(&'static [String]),
    MultiCmd(&'static [&'static [String]]),
    Unicode(char),
    MultiUnicode(&'static [char]),
    Mouse(Btn),
    MultiMouse(&'static [Btn]),
    LiveReload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Btn {
    Left,
    Right,
    Mid,
}
