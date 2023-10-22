//! Safe abstraction over the low-level windows keyboard hook API.

// This file is taken from kbremap with minor modifications.
// https://github.com/timokroeger/kbremap

use std::cell::Cell;
use std::io;
use std::{mem, ptr};

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;

use crate::kanata::CalculatedMouseMove;
use crate::oskbd::{KeyEvent, KeyValue};
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;

type HookFn = dyn FnMut(InputEvent) -> bool;

thread_local! {
    /// Stores the hook callback for the current thread.
    static HOOK: Cell<Option<Box<HookFn>>> = Cell::default();
}

/// Wrapper for the low-level keyboard hook API.
/// Automatically unregisters the hook when dropped.
pub struct KeyboardHook {
    handle: HHOOK,
}

impl KeyboardHook {
    /// Sets the low-level keyboard hook for this thread.
    ///
    /// Panics when a hook is already registered from the same thread.
    #[must_use = "The hook will immediatelly be unregistered and not work."]
    pub fn set_input_cb(callback: impl FnMut(InputEvent) -> bool + 'static) -> KeyboardHook {
        HOOK.with(|state| {
            assert!(
                state.take().is_none(),
                "Only one keyboard hook can be registered per thread."
            );

            state.set(Some(Box::new(callback)));

            KeyboardHook {
                handle: unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), ptr::null_mut(), 0)
                        .as_mut()
                        .expect("install low-level keyboard hook successfully")
                },
            }
        })
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.handle) };
        HOOK.with(|state| state.take());
    }
}

/// Key event received by the low level keyboard hook.
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub code: u32,

    /// Key was released
    pub up: bool,
}

impl InputEvent {
    fn from_hook_lparam(lparam: &KBDLLHOOKSTRUCT) -> Self {
        Self {
            code: lparam.vkCode,
            up: lparam.flags & LLKHF_UP != 0,
        }
    }

    fn from_oscode(code: OsCode, val: KeyValue) -> Self {
        Self {
            code: code.into(),
            up: val.into(),
        }
    }
}

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

impl From<KeyEvent> for InputEvent {
    fn from(item: KeyEvent) -> Self {
        Self {
            code: item.code.into(),
            up: item.value.into(),
        }
    }
}

/// The actual WinAPI compatible callback.
unsafe extern "system" fn hook_proc(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hook_lparam = &*(lparam as *const KBDLLHOOKSTRUCT);
    let is_injected = hook_lparam.flags & LLKHF_INJECTED != 0;
    log::trace!("{code}, {wparam:?}, {is_injected}");
    if code != HC_ACTION {
        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
    }

    let key_event = InputEvent::from_hook_lparam(hook_lparam);

    // `SendInput()` internally calls the hook function. Filter out injected events
    // to prevent recursion and potential stack overflows if our remapping logic
    // sent the injected event.
    if is_injected {
        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
    }

    let mut handled = false;
    HOOK.with(|state| {
        // The unwrap cannot fail, because we have initialized [`HOOK`] with a
        // valid closure before registering the hook (this function).
        // To access the closure we move it out of the cell and put it back
        // after it returned. For this to work we need to prevent recursion by
        // dropping injected events. Otherwise we would try to take the closure
        // twice and the call would fail the second time.
        let mut hook = state.take().expect("no recurse");
        handled = hook(key_event);
        state.set(Some(hook));
    });

    if handled {
        1
    } else {
        CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
    }
}

/// Handle for writing keys to the OS.
pub struct KbdOut {}

impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {})
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        super::send_key_sendinput(event.code as u16, event.up);
        Ok(())
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        let event = InputEvent::from_oscode(key, value);
        self.write(event)
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        super::write_code(code as u16, value)
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    /// Send using VK_PACKET
    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        super::send_uc(c, false);
        super::send_uc(c, true);
        Ok(())
    }

    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        log::debug!("click btn: {:?}", btn);
        match btn {
            Btn::Left => send_btn(MOUSEEVENTF_LEFTDOWN),
            Btn::Right => send_btn(MOUSEEVENTF_RIGHTDOWN),
            Btn::Mid => send_btn(MOUSEEVENTF_MIDDLEDOWN),
            Btn::Backward => send_xbtn(MOUSEEVENTF_XDOWN, XBUTTON1),
            Btn::Forward => send_xbtn(MOUSEEVENTF_XDOWN, XBUTTON2),
        };
        Ok(())
    }

    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        log::debug!("release btn: {:?}", btn);
        match btn {
            Btn::Left => send_btn(MOUSEEVENTF_LEFTUP),
            Btn::Right => send_btn(MOUSEEVENTF_RIGHTUP),
            Btn::Mid => send_btn(MOUSEEVENTF_MIDDLEUP),
            Btn::Backward => send_xbtn(MOUSEEVENTF_XUP, XBUTTON1),
            Btn::Forward => send_xbtn(MOUSEEVENTF_XUP, XBUTTON2),
        };
        Ok(())
    }

    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        log::debug!("scroll: {direction:?} {distance:?}");
        match direction {
            MWheelDirection::Up | MWheelDirection::Down => scroll(direction, distance),
            MWheelDirection::Left | MWheelDirection::Right => hscroll(direction, distance),
        }
        Ok(())
    }

    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        move_mouse(mv.direction, mv.distance);
        Ok(())
    }

    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        move_mouse_many(moves);
        Ok(())
    }

    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        log::info!("setting mouse {x} {y}");
        set_mouse_xy(i32::from(x), i32::from(y));
        Ok(())
    }
}

fn send_btn(flag: u32) {
    unsafe {
        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_MOUSE;

        // set button
        let mut m_input: MOUSEINPUT = mem::zeroed();
        m_input.dwFlags |= flag;

        *inputs[0].u.mi_mut() = m_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}

fn send_xbtn(flag: u32, xbtn: u16) {
    unsafe {
        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_MOUSE;

        // set button
        let mut m_input: MOUSEINPUT = mem::zeroed();
        m_input.dwFlags |= flag;
        m_input.mouseData = xbtn.into();

        *inputs[0].u.mi_mut() = m_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}

fn scroll(direction: MWheelDirection, distance: u16) {
    unsafe {
        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_MOUSE;

        let mut m_input: MOUSEINPUT = mem::zeroed();
        m_input.dwFlags |= MOUSEEVENTF_WHEEL;
        m_input.mouseData = match direction {
            MWheelDirection::Up => distance.into(),
            MWheelDirection::Down => (-i32::from(distance)) as u32,
            _ => unreachable!(), // unreachable based on pub fn scroll
        };

        *inputs[0].u.mi_mut() = m_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}

fn hscroll(direction: MWheelDirection, distance: u16) {
    unsafe {
        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_MOUSE;

        let mut m_input: MOUSEINPUT = mem::zeroed();
        m_input.dwFlags |= MOUSEEVENTF_HWHEEL;
        m_input.mouseData = match direction {
            MWheelDirection::Right => distance.into(),
            MWheelDirection::Left => (-i32::from(distance)) as u32,
            _ => unreachable!(), // unreachable based on pub fn scroll
        };

        *inputs[0].u.mi_mut() = m_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}

fn move_mouse(direction: MoveDirection, distance: u16) {
    log::debug!("move mouse: {direction:?} {distance:?}");
    match direction {
        MoveDirection::Up => move_mouse_xy(0, -i32::from(distance)),
        MoveDirection::Down => move_mouse_xy(0, i32::from(distance)),
        MoveDirection::Left => move_mouse_xy(-i32::from(distance), 0),
        MoveDirection::Right => move_mouse_xy(i32::from(distance), 0),
    }
}

fn move_mouse_many(moves: &[CalculatedMouseMove]) {
    let mut x_acc = 0;
    let mut y_acc = 0;
    for mov in moves {
        let acc_change = match mov.direction {
            MoveDirection::Up => (0, -i32::from(mov.distance)),
            MoveDirection::Down => (0, i32::from(mov.distance)),
            MoveDirection::Left => (-i32::from(mov.distance), 0),
            MoveDirection::Right => (i32::from(mov.distance), 0),
        };
        x_acc += acc_change.0;
        y_acc += acc_change.1;
    }
    move_mouse_xy(x_acc, y_acc);
}

fn move_mouse_xy(x: i32, y: i32) {
    mouse_event(MOUSEEVENTF_MOVE, 0, x, y);
}

fn set_mouse_xy(x: i32, y: i32) {
    mouse_event(
        MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE | MOUSEEVENTF_VIRTUALDESK,
        0,
        x,
        y,
    );
}

// Taken from Enigo: https://github.com/enigo-rs/enigo
fn mouse_event(flags: u32, data: u32, dx: i32, dy: i32) {
    let mut input = INPUT {
        type_: INPUT_MOUSE,
        u: unsafe {
            mem::transmute(MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            })
        },
    };
    unsafe { SendInput(1, &mut input as LPINPUT, mem::size_of::<INPUT>() as c_int) };
}
