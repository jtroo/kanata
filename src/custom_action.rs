#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomAction {
    Unicode(char),
    Mouse(Btn),
    LiveReload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Btn {
    Left,
    Right,
    Mid,
}
