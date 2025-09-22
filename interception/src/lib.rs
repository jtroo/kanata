pub extern crate interception_sys;

#[macro_use]
extern crate bitflags;

pub use interception_sys as raw;
pub mod scancode;

pub use scancode::ScanCode;

use std::convert::{TryFrom, TryInto};
use std::default::Default;
use std::time::Duration;
use std::vec::Vec;

pub type Device = i32;
pub type Precedence = i32;

pub enum Filter {
    MouseFilter(MouseFilter),
    KeyFilter(KeyFilter),
}

pub type Predicate = extern "C" fn(device: Device) -> bool;

bitflags! {
    pub struct MouseState: u16 {
        const LEFT_BUTTON_DOWN = 1;
        const LEFT_BUTTON_UP = 2;

        const RIGHT_BUTTON_DOWN = 4;
        const RIGHT_BUTTON_UP = 8;

        const MIDDLE_BUTTON_DOWN = 16;
        const MIDDLE_BUTTON_UP = 32;

        const BUTTON_4_DOWN = 64;
        const BUTTON_4_UP = 128;

        const BUTTON_5_DOWN = 256;
        const BUTTON_5_UP = 512;

        const WHEEL = 1024;
        const HWHEEL = 2048;

        // MouseFilter only
        const MOVE = 4096;
    }
}

pub type MouseFilter = MouseState;

bitflags! {
    pub struct MouseFlags: u16 {
        const MOVE_RELATIVE = 0;
        const MOVE_ABSOLUTE = 1;

        const VIRTUAL_DESKTOP = 2;
        const ATTRIBUTES_CHANGED = 4;

        const MOVE_NO_COALESCE = 8;

        const TERMSRV_SRC_SHADOW = 256;
    }
}

bitflags! {
    pub struct KeyState: u16 {
        const DOWN = 0;
        const UP = 1;

        const E0 = 2;
        const E1 = 3;

        const TERMSRV_SET_LED = 8;
        const TERMSRV_SHADOW = 16;
        const TERMSRV_VKPACKET = 32;
    }
}

bitflags! {
    pub struct KeyFilter: u16 {
        const DOWN = 1;
        const UP = 2;

        const E0 = 4;
        const E1 = 8;

        const TERMSRV_SET_LED = 16;
        const TERMSRV_SHADOW = 32;
        const TERMSRV_VKPACKET = 64;
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Stroke {
    Mouse {
        state: MouseState,
        flags: MouseFlags,
        rolling: i16,
        x: i32,
        y: i32,
        information: u32,
    },

    Keyboard {
        code: ScanCode,
        state: KeyState,
        information: u32,
    },
}

impl TryFrom<raw::InterceptionMouseStroke> for Stroke {
    type Error = &'static str;

    fn try_from(raw_stroke: raw::InterceptionMouseStroke) -> Result<Self, Self::Error> {
        let state = match MouseState::from_bits(raw_stroke.state) {
            Some(state) => state,
            None => return Err("Extra bits in raw mouse state"),
        };

        let flags = match MouseFlags::from_bits(raw_stroke.flags) {
            Some(flags) => flags,
            None => return Err("Extra bits in raw mouse flags"),
        };

        Ok(Stroke::Mouse {
            state: state,
            flags: flags,
            rolling: raw_stroke.rolling,
            x: raw_stroke.x,
            y: raw_stroke.y,
            information: raw_stroke.information,
        })
    }
}

impl TryFrom<raw::InterceptionKeyStroke> for Stroke {
    type Error = &'static str;

    fn try_from(raw_stroke: raw::InterceptionKeyStroke) -> Result<Self, Self::Error> {
        let state = match KeyState::from_bits(raw_stroke.state) {
            Some(state) => state,
            None => return Err("Extra bits in raw keyboard state"),
        };

        let code = match ScanCode::try_from(raw_stroke.code) {
            Ok(code) => code,
            Err(_) => ScanCode::Esc,
        };

        Ok(Stroke::Keyboard {
            code: code,
            state: state,
            information: raw_stroke.information,
        })
    }
}

impl TryFrom<Stroke> for raw::InterceptionMouseStroke {
    type Error = &'static str;

    fn try_from(stroke: Stroke) -> Result<Self, Self::Error> {
        if let Stroke::Mouse {
            state,
            flags,
            rolling,
            x,
            y,
            information,
        } = stroke
        {
            Ok(raw::InterceptionMouseStroke {
                state: state.bits(),
                flags: flags.bits(),
                rolling: rolling,
                x: x,
                y: y,
                information: information,
            })
        } else {
            Err("Stroke must be a mouse stroke")
        }
    }
}

impl TryFrom<Stroke> for raw::InterceptionKeyStroke {
    type Error = &'static str;

    fn try_from(stroke: Stroke) -> Result<Self, Self::Error> {
        if let Stroke::Keyboard {
            code,
            state,
            information,
        } = stroke
        {
            Ok(raw::InterceptionKeyStroke {
                code: code as u16,
                state: state.bits(),
                information: information,
            })
        } else {
            Err("Stroke must be a keyboard stroke")
        }
    }
}

pub struct Interception {
    ctx: raw::InterceptionContext,
}

impl Interception {
    pub fn new() -> Option<Self> {
        let ctx = unsafe { raw::interception_create_context() };

        if ctx == std::ptr::null_mut() {
            return None;
        }

        Some(Interception { ctx: ctx })
    }

    pub fn get_precedence(&self, device: Device) -> Precedence {
        unsafe { raw::interception_get_precedence(self.ctx, device) }
    }

    pub fn set_precedence(&self, device: Device, precedence: Precedence) {
        unsafe { raw::interception_set_precedence(self.ctx, device, precedence) }
    }

    pub fn get_filter(&self, device: Device) -> Filter {
        if is_invalid(device) {
            return Filter::KeyFilter(KeyFilter::empty());
        }

        let raw_filter = unsafe { raw::interception_get_filter(self.ctx, device) };
        if is_mouse(device) {
            let filter = match MouseFilter::from_bits(raw_filter) {
                Some(filter) => filter,
                None => MouseFilter::empty(),
            };

            Filter::MouseFilter(filter)
        } else {
            let filter = match KeyFilter::from_bits(raw_filter) {
                Some(filter) => filter,
                None => KeyFilter::empty(),
            };

            Filter::KeyFilter(filter)
        }
    }

    pub fn set_filter(&self, predicate: Predicate, filter: Filter) {
        let filter = match filter {
            Filter::MouseFilter(filter) => filter.bits(),
            Filter::KeyFilter(filter) => filter.bits(),
        };

        unsafe {
            let predicate = std::mem::transmute(Some(predicate));
            raw::interception_set_filter(self.ctx, predicate, filter)
        }
    }

    pub fn wait(&self) -> Device {
        unsafe { raw::interception_wait(self.ctx) }
    }

    pub fn wait_with_timeout(&self, duration: Duration) -> Device {
        let millis = match u32::try_from(duration.as_millis()) {
            Ok(m) => m,
            Err(_) => u32::MAX,
        };

        unsafe { raw::interception_wait_with_timeout(self.ctx, millis) }
    }

    pub fn send(&self, device: Device, strokes: &[Stroke]) -> i32 {
        if is_mouse(device) {
            self.send_internal::<raw::InterceptionMouseStroke>(device, strokes)
        } else if is_keyboard(device) {
            self.send_internal::<raw::InterceptionKeyStroke>(device, strokes)
        } else {
            0
        }
    }

    fn send_internal<T: TryFrom<Stroke>>(&self, device: Device, strokes: &[Stroke]) -> i32 {
        let mut raw_strokes = Vec::new();

        for stroke in strokes {
            if let Ok(raw_stroke) = T::try_from(*stroke) {
                raw_strokes.push(raw_stroke)
            }
        }

        let ptr = raw_strokes.as_ptr();
        let len = match u32::try_from(raw_strokes.len()) {
            Ok(l) => l,
            Err(_) => u32::MAX,
        };

        unsafe { raw::interception_send(self.ctx, device, std::mem::transmute(ptr), len) }
    }

    pub fn receive(&self, device: Device, strokes: &mut [Stroke]) -> i32 {
        if is_mouse(device) {
            self.receive_internal::<raw::InterceptionMouseStroke>(device, strokes)
        } else if is_keyboard(device) {
            self.receive_internal::<raw::InterceptionKeyStroke>(device, strokes)
        } else {
            0
        }
    }

    fn receive_internal<T: TryInto<Stroke> + Default + Copy>(
        &self,
        device: Device,
        strokes: &mut [Stroke],
    ) -> i32 {
        let mut raw_strokes: Vec<T> = Vec::with_capacity(strokes.len());
        raw_strokes.resize_with(strokes.len(), Default::default);

        let ptr = raw_strokes.as_ptr();
        let len = match u32::try_from(raw_strokes.len()) {
            Ok(l) => l,
            Err(_) => u32::MAX,
        };

        let num_read =
            unsafe { raw::interception_receive(self.ctx, device, std::mem::transmute(ptr), len) };

        let mut num_valid: i32 = 0;
        for i in 0..num_read {
            if let Ok(stroke) = raw_strokes[i as usize].try_into() {
                strokes[num_valid as usize] = stroke;
                num_valid += 1;
            }
        }

        num_valid
    }

    pub fn get_hardware_id(&self, device: Device, buffer: &mut [u8]) -> u32 {
        let ptr = buffer.as_mut_ptr();
        let len = match u32::try_from(buffer.len()) {
            Ok(l) => l,
            Err(_) => u32::MAX,
        };

        unsafe {
            raw::interception_get_hardware_id(self.ctx, device, std::mem::transmute(ptr), len)
        }
    }
}

impl Drop for Interception {
    fn drop(&mut self) {
        unsafe { raw::interception_destroy_context(self.ctx) }
    }
}

pub extern "C" fn is_invalid(device: Device) -> bool {
    unsafe { raw::interception_is_invalid(device) != 0 }
}

pub extern "C" fn is_keyboard(device: Device) -> bool {
    unsafe { raw::interception_is_keyboard(device) != 0 }
}

pub extern "C" fn is_mouse(device: Device) -> bool {
    unsafe { raw::interception_is_mouse(device) != 0 }
}
