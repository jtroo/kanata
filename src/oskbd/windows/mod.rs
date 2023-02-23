use std::mem;

use winapi::ctypes::*;
use winapi::um::winuser::*;

use encode_unicode::CharExt;

use crate::custom_action::*;
use crate::keys::KeyValue;

#[cfg(not(feature = "interception_driver"))]
mod llhook;
#[cfg(not(feature = "interception_driver"))]
pub use llhook::*;

#[cfg(feature = "interception_driver")]
mod interception;
#[cfg(feature = "interception_driver")]
mod interception_convert;
#[cfg(feature = "interception_driver")]
pub use self::interception::*;
#[cfg(feature = "interception_driver")]
pub use interception_convert::*;

fn send_uc(c: char, up: bool) {
    let mut inputs: [INPUT; 2] = unsafe { mem::zeroed() };

    let n_inputs = inputs
        .iter_mut()
        .zip(c.to_utf16())
        .map(|(input, c)| {
            let mut kb_input: KEYBDINPUT = unsafe { mem::zeroed() };
            kb_input.wScan = c;
            kb_input.dwFlags |= KEYEVENTF_UNICODE;
            if up {
                kb_input.dwFlags |= KEYEVENTF_KEYUP;
            }
            input.type_ = INPUT_KEYBOARD;
            unsafe { *input.u.ki_mut() = kb_input };
        })
        .count();

    unsafe {
        SendInput(
            n_inputs as _,
            inputs.as_mut_ptr(),
            mem::size_of::<INPUT>() as _,
        );
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

fn move_mouse_xy(x: i32, y: i32) {
    mouse_event(MOUSEEVENTF_MOVE, 0, x, y);
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

fn write_code(code: u16, value: KeyValue) -> Result<(), std::io::Error> {
    send_key_sendinput(
        code,
        match value {
            KeyValue::Press | KeyValue::Repeat => false,
            KeyValue::Release => true,
        },
    );
    Ok(())
}

fn send_key_sendinput(code: u16, is_key_up: bool) {
    unsafe {
        let mut kb_input: KEYBDINPUT = mem::zeroed();
        if is_key_up {
            kb_input.dwFlags |= KEYEVENTF_KEYUP;
        }
        kb_input.wVk = code;

        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = kb_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}
