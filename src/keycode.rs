use evdev_rs::enums::EV_KEY;

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
pub struct KeyCode {
    pub c: u32,
}

impl From<usize> for KeyCode {
    fn from(item: usize) -> Self {
        Self{c: item as u32}
    }
}

impl From<EV_KEY> for KeyCode {
    fn from(item: EV_KEY) -> Self {
        Self{c: item as u32}
    }
}

impl From<KeyCode> for usize {
    fn from(item: KeyCode) -> Self {
        item.c as usize
    }
}

impl From<KeyCode> for EV_KEY {
    fn from(item: KeyCode) -> Self {
        evdev_rs::enums::int_to_ev_key(item.c)
            .expect(&format!("Invalid KeyCode: {}", item.c))
    }
}
