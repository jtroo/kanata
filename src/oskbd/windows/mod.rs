#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

use std::mem;

use winapi::um::winuser::*;

use encode_unicode::CharExt;

use crate::oskbd::KeyValue;

#[cfg(not(feature = "interception_driver"))]
mod llhook;
#[cfg(not(feature = "interception_driver"))]
pub use llhook::*;

mod scancode_to_usvk;
#[allow(unused)]
pub use scancode_to_usvk::*;

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
            /*
            Credit to @VictorLemosR from GitHub for the code here 🙂:

            All the keys that are extended are on font 1, inside the table on column 'Scan 1 Make' and start with '0xE0'.
            To obtain the scancode, one could just print 'kb_input.wScan' from the function below.
            Font 1: https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input#scan-codes
            To obtain a virtual key code, one could just print 'code' from the function below for a key or see font 2
            Font 2: https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes

            For example, left arrow and 4 from numpad. For the scancode, they have the same low byte,
            but not the same high byte, which is 0xE0. LeftArrow = 0xE04B, keypad 4 = 0x004B. For the virtual code,
            left arrow is 0x25 and 4 from numpad is 0x64.
            There is a windows function called 'MapVirtualKeyA' that can be used to convert a virtual key code to a scancode.
            */
            const EXTENDED_KEYS: [u8; 48] = [
                0xb1, 0xb0, 0xa3, 0xad, 0x8c, 0xb3, 0xb2, 0xae, 0xaf, 0xac, 0x6f, 0x2c, 0xa5, 0x24,
                0x26, 0x21, 0x25, 0x27, 0x23, 0x28, 0x22, 0x2d, 0x2e, 0x5b, 0x5c, 0x5d, 0x5f, 0xaa,
                0xa8, 0xa9, 0xa7, 0xa6, 0xac, 0xb4, 0x13,
                /*
                The 0x13 here is repeated. Why? Maybe it will generate better comparison code 😅.
                Probably should test+measure when making changes like this (but I didn't).
                The theory is that comparing on a 16-byte boundary seems good.
                Below taken from Rust source:

                const fn memchr_aligned(x: u8, text: &[u8]) -> Option<usize> {
                    // Scan for a single byte value by reading two `usize` words at a time.
                    //
                    // Split `text` in three parts
                    // - unaligned initial part, before the first word aligned address in text
                    // - body, scan by 2 words at a time
                    // - the last remaining part, < 2 word size
                */
                0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13, 0x13,
            ];

            let code_u32 = code as u32;
            kb_input.dwFlags |= KEYEVENTF_SCANCODE;
            #[cfg(not(feature = "win_llhook_read_scancodes"))]
            {
                kb_input.wScan = MapVirtualKeyA(code_u32, 0) as u16;
            }
            #[cfg(feature = "win_llhook_read_scancodes")]
            {
                kb_input.wScan =
                    osc_to_u16(code.into()).unwrap_or_else(|| MapVirtualKeyA(code_u32, 0) as u16);
            }
            if kb_input.wScan == 0 {
                kb_input.dwFlags &= !KEYEVENTF_SCANCODE;
                kb_input.wVk = code;
            }

            let is_extended_key: bool = code < 0xff && EXTENDED_KEYS.contains(&(code as u8));
            if is_extended_key {
                kb_input.wScan |= 0xE0 << 8;
                kb_input.dwFlags |= KEYEVENTF_EXTENDEDKEY;
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
