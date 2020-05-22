use evdev_rs::enums::{EV_KEY, EventCode};
use evdev_rs::{InputEvent, TimeVal};

// ------------------ KeyCode --------------------

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

impl From<EventCode> for KeyCode {
    fn from(item: EventCode) -> Self {
        match item {
            EventCode::EV_KEY(evkey) => Self::from(evkey),
            _ => { assert!(false); 0.into() }
        }
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

// ------------------ KeyValue --------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyValue {
    Release = 0,
    Press = 1,
    Repeat = 2,
}

impl From<i32> for KeyValue {
    fn from(item: i32) -> Self {
        match item {
            0 => Self::Release,
            1 => Self::Press,
            2 => Self::Repeat,
            _ => {
                assert!(false);
                Self::Release
            }
        }
    }
}

// ------------------ KeyEvent --------------------

pub struct KeyEvent {
    pub event: InputEvent,
}

impl KeyEvent {
    pub fn new(code: &EventCode, value: i32) -> Self {
        let time = TimeVal::new(0, 0);
        let event = InputEvent::new(&time, code, value);
        Self{event}
    }

    #[cfg(test)]
    pub fn new_press(code: &EventCode) -> Self {
        Self::new(code, KeyValue::Press as i32)
    }

    #[cfg(test)]
    pub fn new_release(code: &EventCode) -> Self {
        Self::new(code, KeyValue::Release as i32)
    }
}
