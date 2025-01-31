//! Safe abstraction over the low-level windows keyboard hook API.

// This file is taken from kbremap with minor modifications.
// https://github.com/timokroeger/kbremap

#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

use core::fmt;
use std::cell::Cell;
use std::io;
use std::{mem, ptr};

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winuser::*;

use crate::kanata::CalculatedMouseMove;
use crate::oskbd::{KeyEvent, KeyValue};
use kanata_keyberon::key_code::KeyCode;
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;

pub const LLHOOK_IDLE_TIME_SECS_CLEAR_INPUTS: u64 = 60;

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

impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let direction = if self.up { "↑" } else { "↓" };
        let key_name = KeyCode::from(OsCode::from(self.code));
        write!(f, "{}{:?}", direction, key_name)
    }
}

impl InputEvent {
    #[rustfmt::skip]
    fn from_hook_lparam(lparam: &KBDLLHOOKSTRUCT) -> Self {
        let code = if lparam.vkCode == (VK_RETURN as u32) {
            match lparam.flags & 0x1 {
                0 => VK_RETURN as u32,
                _ => u32::from(VK_KPENTER_FAKE),
            }
        } else {
            #[cfg(not(feature = "win_llhook_read_scancodes"))]
            {
                lparam.vkCode
            }
            #[cfg(feature = "win_llhook_read_scancodes")]
            {
                let extended = if lparam.flags & 0x1 == 0x1 {
                    0xE000
                } else {
                    0
                };
                crate::oskbd::u16_to_osc((lparam.scanCode as u16) | extended)
                    .map(Into::into)
                    .unwrap_or(lparam.vkCode)
            }
        };
        Self {
            code,
            up: lparam.flags & LLKHF_UP != 0,
        }
    }

    pub fn from_oscode(code: OsCode, val: KeyValue) -> Self {
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
/// code: determines how to process the message
/// source: https://learn.microsoft.com/windows/win32/winmsg/lowlevelkeyboardproc
///   <0 : must pass the message to CallNextHookEx without further processing
///    and should return the value returned by CallNextHookEx
///   HC_ACTION (=0) : wParam and lParam parameters contain information about the message
///
/// wparam: ID keyboard message
/// source: https://learn.microsoft.com/windows/win32/winmsg/lowlevelkeyboardproc
///   WM_KEY(DOWN|UP) Posted to kb-focused window when a nonsystem key is pressed
///   WM_SYSKEYDOWN¦UP Posted to kb-focused window when a F10 (activate menu bar)
///     or ⎇X⃣ or posted to active window if no win has kb focus (check context code in lParam)
///
/// lparam: pointer to a KBDLLHOOKSTRUCT struct
/// source: https://learn.microsoft.com/windows/win32/api/winuser/ns-winuser-kbdllhookstruct
///   vkCode     :DWORD key's virtual code (1–254)
///   scanCode   :DWORD key's hardware scan code
///   flags      :DWORD flags (extended-key, event-injected, transition-state), context code
///     Bits (2-3 6 reserved)                        Description
///     7 KF_UP       >> 8 LLKHF_UP                  transition state: 0=key↓  1=key↑
///                                                           (being pressed)  (being released)
///     5 KF_ALTDOWN  >> 8 LLKHF_ALTDOWN             context code    : 1=alt↓  0=alt↑
///     4 0x10             LLKHF_INJECTED            event was injected: 1=yes, 0=no
///     1 0x02             LLKHF_LOWER_IL_INJECTED   injected by proc with lower integrity level
//                                                   1=yes 0=no (bit 4 will also set)
///     0 KF_EXTENDED >> 8 LLKHF_EXTENDED            extended key (Fn, numpad): 1=yes, 0=no
///   time       :DWORD time stamp = GetMessageTime
///   dwExtraInfo:ULONG_PTR Additional info
unsafe extern "system" fn hook_proc(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hook_lparam = &*(lparam as *const KBDLLHOOKSTRUCT);
    let is_injected = hook_lparam.flags & LLKHF_INJECTED != 0;
    log::trace!("{code} {}{wparam} {is_injected}", {
        match wparam as u32 {
            WM_KEYDOWN => "↓",
            WM_KEYUP => "↑",
            WM_SYSKEYDOWN => "sys↓",
            WM_SYSKEYUP => "sys↑",
            _ => "?",
        }
    });

    // Regarding code check:
    // If code is non-zero (technically <0, but 0 is the only valid value anyway),
    // then it must be forwarded.
    // Source: https://learn.microsoft.com/windows/win32/winmsg/lowlevelkeyboardproc
    //
    // Regarding in_injected check:
    // `SendInput()` internally calls the hook function.
    // Filter out injected events to prevent infinite recursion.
    if code != HC_ACTION || is_injected {
        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
    }

    let key_event = InputEvent::from_hook_lparam(hook_lparam);

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

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
/// Handle for writing keys to the OS.
pub struct KbdOut {}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
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

    pub fn write_code_raw(&mut self, code: u16, value: KeyValue) -> Result<(), io::Error> {
        super::write_code_raw(code, value)
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
            mem::transmute::<winapi::um::winuser::MOUSEINPUT, winapi::um::winuser::INPUT_u>(
                MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            )
        },
    };
    unsafe { SendInput(1, &mut input as LPINPUT, mem::size_of::<INPUT>() as c_int) };
}
