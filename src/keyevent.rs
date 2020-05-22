use evdev_rs::enums::EventCode;
use evdev_rs::InputEvent;
use evdev_rs::TimeVal;

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

pub struct KeyEvent {
    pub event: InputEvent,
}

impl KeyEvent {
    pub fn new(code: &EventCode, value: i32) -> Self {
        let time = TimeVal::new(0, 0);
        let event = InputEvent::new(&time, code, value);
        Self{event}
    }

    pub fn new_press(code: &EventCode) -> Self {
        Self::new(code, KeyValue::Press as i32)
    }

    pub fn new_release(code: &EventCode) -> Self {
        Self::new(code, KeyValue::Release as i32)
    }

}
