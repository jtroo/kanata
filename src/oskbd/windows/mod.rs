use std::mem;

use winapi::um::winuser::*;

use encode_unicode::CharExt;

use crate::oskbd::KeyValue;

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

pub const HI_RES_SCROLL_UNITS_IN_LO_RES: u16 = 120;

fn send_uc(c: char, up: bool) {
    log::debug!("sending unicode {c}");
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

fn write_code(code: u16, value: KeyValue) -> Result<(), std::io::Error> {
    send_key_sendinput(
        code,
        match value {
            KeyValue::Press | KeyValue::Repeat => false,
            KeyValue::Release => true,
            KeyValue::Tap => panic!("invalid value attempted to be sent"),
            KeyValue::WakeUp => panic!("invalid value attempted to be sent"),
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

        #[cfg(feature = "win_sendinput_send_scancodes")]
        {
            // GUI/Windows keys don't seem to like SCANCODE events so don't transform those.
            if i32::from(code) == VK_RWIN || i32::from(code) == VK_LWIN {
                kb_input.wVk = code;
            } else {
                let code_u32 = code as u32;
                kb_input.dwFlags |= KEYEVENTF_SCANCODE;
                kb_input.wScan = MapVirtualKeyA(code_u32, 0) as u16;
            }
        }
        #[cfg(not(feature = "win_sendinput_send_scancodes"))]
        {
            kb_input.wVk = code;
        }

        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = kb_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}
