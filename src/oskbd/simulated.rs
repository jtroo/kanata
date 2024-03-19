//! Output that just prints text to stdout instead of actually doing anything OS-related.
/*
todo: add output ticks
A sim.txt file:
↓:j 🕐:1500 ↓:l 🕐:5000 ↓:1 🕐:50 ↑:1 🕐:50 ↓:1 🕐:50 ↑:1 🕐:50 ↑:j 🕐:50 ↑:l 🕐:50
Will print the following output for a config with J,L mapped to home row mods tap-hold-release and 1 printing 🔣🤲🏿
(not that Σin is identical to the sim.txt)
🕐Δms│     1500     5000                     50           50     50           50     50             50
In ———————————————————————————————————————————————————————————————————————————————————————————————————————————————————————————————————
 k↑ │                                                 1                   1      J              L
 k↓ │  J        L                        1                   1
 k⟳ │
Σin │ ↓J 🕐1500 ↓L 🕐5000                 ↓1 🕐50       ↑1 🕐50 ↓1 🕐50       ↑1 🕐50 ↑J 🕐50         ↑L 🕐50
Out———————————————————————————————————————————————————————————————————————————————————————————————————————————————————————————————————
 k↑ │                                                                                   ⇧›             ⎇›
 k↓ │                    ⇧›      ⎇›
 🖰↑ │
 🖰↓ │
 🖰  │
 🔣  │                                           🤲                   🤲
 code│
 raw↑│
 raw↓│
Σout│                   ↓⇧›     ↓⎇›             🤲                   🤲                  ↑⇧›            ↑⎇›
*/
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LogFmtT {
    KeyUp,
    KeyDown,
    MouseUp,
    MouseDown,
    MouseMove,
    Unicode,
    Code,
    RawUp,
    RawDown,
    InKeyUp,
    InKeyDown,
    InKeyRep,
    InTick,
}

pub struct LogFmt {
    time: String,
    //In       	//
    in_key_up: String,
    in_key_down: String,
    in_key_rep: String,
    in_combo: String,
    //Out      	//
    key_up: String,
    key_down: String,
    raw_up: String,
    raw_down: String,
    combo: String,
    mouse_up: String,
    mouse_down: String,
    mouse_move: String,
    unicode: String,
    code: String,
}
impl Default for LogFmt {
    fn default() -> Self {
        Self::new()
    }
}
impl LogFmt {
    pub fn new() -> Self {
        Self {
            time: String::new(),
            //In       	//
            in_key_up: String::new(),
            in_key_down: String::new(),
            in_key_rep: String::new(),
            in_combo: String::new(),
            //Out      	//
            key_up: String::new(),
            key_down: String::new(),
            raw_up: String::new(),
            raw_down: String::new(),
            mouse_up: String::new(),
            mouse_down: String::new(),
            mouse_move: String::new(),
            unicode: String::new(),
            code: String::new(),
            combo: String::new(),
        }
    }
    pub fn fmt(&mut self, key: LogFmtT, value: String) {
        let pad = value.len();
        let blank = format!("  {: <pad$}", ""); //+␠ to allow combo log indicator
        let val = format!("  {: <pad$}", value);
        self.time += if key == LogFmtT::InTick {
            self.combo += &blank;
            self.in_combo += &format!(" 🕐{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.in_key_up += if key == LogFmtT::InKeyUp {
            self.combo += &blank;
            self.in_combo += &format!(" ↑{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.in_key_down += if key == LogFmtT::InKeyDown {
            self.combo += &blank;
            self.in_combo += &format!(" ↓{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.in_key_rep += if key == LogFmtT::InKeyRep {
            self.combo += &blank;
            self.in_combo += &format!(" ⟳{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.key_up += if key == LogFmtT::KeyUp {
            self.in_combo += &blank;
            self.combo += &format!(" ↑{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.key_down += if key == LogFmtT::KeyDown {
            self.in_combo += &blank;
            self.combo += &format!(" ↓{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.mouse_up += if key == LogFmtT::MouseUp {
            self.in_combo += &blank;
            self.combo += &format!(" ↑{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.mouse_down += if key == LogFmtT::MouseDown {
            self.in_combo += &blank;
            self.combo += &format!(" ↓{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.mouse_move += if key == LogFmtT::MouseMove {
            self.in_combo += &blank;
            self.combo += &val;
            &val
        } else {
            &blank
        };
        self.raw_up += if key == LogFmtT::RawUp {
            self.in_combo += &blank;
            self.combo += &format!(" ↑{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.raw_down += if key == LogFmtT::RawDown {
            self.in_combo += &blank;
            self.combo += &format!(" ↓{: <pad$}", value);
            &val
        } else {
            &blank
        };
        self.unicode += if key == LogFmtT::Unicode {
            self.in_combo += &blank;
            self.combo += &val;
            &val
        } else {
            &blank
        };
        self.code += if key == LogFmtT::Code {
            self.in_combo += &blank;
            self.combo += &val;
            &val
        } else {
            &blank
        };
    }

    pub fn write_raw(&mut self, event: InputEvent) {
        let key_name = KeyCode::from(OsCode::from(event.code));
        if event.up {
            self.fmt(LogFmtT::RawUp, key_name.to_string())
        } else {
            self.fmt(LogFmtT::RawDown, key_name.to_string())
        }
    }
    pub fn in_tick(&mut self, t: u128) {
        self.fmt(LogFmtT::InTick, t.to_string())
    }
    pub fn in_press_key(&mut self, key: OsCode) {
        self.fmt(LogFmtT::InKeyDown, KeyCode::from(key).to_string())
    }
    pub fn in_release_key(&mut self, key: OsCode) {
        self.fmt(LogFmtT::InKeyUp, KeyCode::from(key).to_string())
    }
    pub fn in_repeat_key(&mut self, key: OsCode) {
        self.fmt(LogFmtT::InKeyRep, KeyCode::from(key).to_string())
    }
    pub fn press_key(&mut self, key: OsCode) {
        self.fmt(LogFmtT::KeyDown, KeyCode::from(key).to_string())
    }
    pub fn release_key(&mut self, key: OsCode) {
        self.fmt(LogFmtT::KeyUp, KeyCode::from(key).to_string())
    }
    pub fn send_unicode(&mut self, c: char) {
        self.fmt(LogFmtT::Unicode, c.to_string())
    }
    pub fn click_btn(&mut self, btn: Btn) {
        self.fmt(LogFmtT::MouseDown, btn.to_string())
    }
    pub fn release_btn(&mut self, btn: Btn) {
        self.fmt(LogFmtT::MouseUp, btn.to_string())
    }
    pub fn set_mouse(&mut self, x: u16, y: u16) {
        self.fmt(LogFmtT::MouseMove, format!("@{},{}", x, y))
    }
    pub fn scroll(&mut self, dir: MWheelDirection, dist: u16) {
        self.fmt(LogFmtT::MouseMove, format!("{}{}", dir, dist))
    }
    pub fn move_mouse(&mut self, dir: MoveDirection, dist: u16) {
        self.fmt(LogFmtT::MouseMove, format!("{}{}", dir, dist))
    }
    pub fn write_code(&mut self, code: u32, value: KeyValue) {
        self.fmt(LogFmtT::Code, format!("{code};{value:?}"))
    }

    pub fn end(&self) {
        let pad = self.combo.len() - 3;
        println!("🕐Δms│{}", self.time);
        println!("In {:—<pad$}", "");
        println!(" k↑ │{}", self.in_key_up);
        println!(" k↓ │{}", self.in_key_down);
        println!(" k⟳ │{}", self.in_key_rep);
        println!("Σin │{}", self.in_combo);
        println!("Out{:—<pad$}", "");
        println!(" k↑ │{}", self.key_up);
        println!(" k↓ │{}", self.key_down);
        println!(" 🖰↑ │{}", self.mouse_up);
        println!(" 🖰↓ │{}", self.mouse_down);
        println!(" 🖰  │{}", self.mouse_move);
        println!(" 🔣  │{}", self.unicode);
        println!(" code│{}", self.code);
        println!(" raw↑│{}", self.raw_up);
        println!(" raw↓│{}", self.raw_down);
        println!("Σout│{}", self.combo);
    }
}

use super::*;

use crate::kanata::CalculatedMouseMove;
use kanata_parser::custom_action::*;

use std::io;

use kanata_keyberon::key_code::KeyCode;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::fmt;

/// Handle for writing keys to the OS.
pub struct KbdOut {
    pub log: LogFmt,
}

impl KbdOut {
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self { log: LogFmt::new() })
    }
    #[cfg(target_os = "linux")]
    pub fn new(_: &Option<String>) -> Result<Self, io::Error> {
        Ok(Self {})
    }
    #[cfg(target_os = "linux")]
    pub fn write_raw(&mut self, event: InputEvent) -> Result<(), io::Error> {
        self.log.write_raw(event);
        println!("out-raw:{event:?}");
        Ok(())
    }
    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        println!("out:{event}");
        Ok(())
    }
    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let key_ev = KeyEvent::new(key, value);
        let event = {
            #[cfg(target_os = "macos")]
            {
                key_ev.try_into().unwrap()
            }
            #[cfg(not(target_os = "macos"))]
            {
                key_ev.into()
            }
        };
        self.write(event)
    }
    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        self.log.write_code(code, value);
        println!("out-code:{code};{value:?}");
        Ok(())
    }
    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.log.press_key(key);
        self.write_key(key, KeyValue::Press)
    }
    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.log.release_key(key);
        self.write_key(key, KeyValue::Release)
    }
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        self.log.send_unicode(c);
        println!("outU:{c}");
        Ok(())
    }
    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        self.log.click_btn(btn);
        println!("out🖰:↓{btn:?}");
        Ok(())
    }
    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        self.log.release_btn(btn);
        println!("out🖰:↑{btn:?}");
        Ok(())
    }
    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        println!("scroll:{direction:?},{distance:?}");
        Ok(())
    }
    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        let (direction, distance) = (mv.direction, mv.distance);
        self.log.move_mouse(direction, distance);
        println!("out🖰:move {direction:?},{distance:?}");
        Ok(())
    }
    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        for mv in moves {
            let (direction, distance) = (&mv.direction, &mv.distance);
            self.log.move_mouse(*direction, *distance);
            println!("out🖰:move {direction:?},{distance:?}");
        }
        Ok(())
    }
    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        self.log.set_mouse(x, y);
        log::info!("out🖰:@{x},{y}");
        Ok(())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub code: u32,

    /// Key was released
    pub up: bool,
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let direction = if self.up { "↑" } else { "↓" };
        let key_name = KeyCode::from(OsCode::from(self.code));
        write!(f, "{}{:?}", direction, key_name)
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl InputEvent {
    pub fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        Self {
            code: code.into(),
            up: val.into(),
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();
    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        Ok(Self {
            code: OsCode::from_u16(item.code as u16).ok_or(())?,
            value: match item.up {
                true => KeyValue::Release,
                false => KeyValue::Press,
            },
        })
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        Self {
            code: item.code.into(),
            up: item.value.into(),
        }
    }
}
#[cfg(target_os = "macos")]
impl KeyEvent {
    pub fn new(code: OsCode, value: KeyValue) -> Self {
        Self { code, value }
    }
}
