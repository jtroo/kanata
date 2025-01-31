#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

#[cfg(not(feature = "simulated_input"))]
use std::mem;

#[cfg(not(feature = "simulated_input"))]
use winapi::um::winuser::*;

#[cfg(not(feature = "simulated_input"))]
use encode_unicode::CharExt;

#[cfg(not(feature = "simulated_input"))]
use crate::oskbd::KeyValue;

#[cfg(all(not(feature = "interception_driver"), not(feature = "simulated_input")))]
mod llhook; // contains KbdOut any(not(feature = "simulated_output"), not(feature = "passthru_ahk"))
#[cfg(all(not(feature = "interception_driver"), not(feature = "simulated_input")))]
pub use llhook::*;

#[cfg(all(not(feature = "interception_driver"), feature = "simulated_input"))]
mod exthook_os;
#[cfg(all(not(feature = "interception_driver"), feature = "simulated_input"))]
pub use exthook_os::*;

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

#[cfg(not(feature = "simulated_input"))]
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

#[cfg(not(feature = "simulated_output"))]
fn write_code_raw(code: u16, value: KeyValue) -> Result<(), std::io::Error> {
    let is_key_up = match value {
        KeyValue::Press | KeyValue::Repeat => false,
        KeyValue::Release => true,
        KeyValue::Tap => panic!("invalid value attempted to be sent"),
        KeyValue::WakeUp => panic!("invalid value attempted to be sent"),
    };
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
    Ok(())
}

#[cfg(not(feature = "simulated_input"))]
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

#[cfg(not(feature = "simulated_input"))]
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
                // This MapVirtualKeyA is needed to translate back to the proper scancode
                // associated with with the virtual key.
                // E.g. take this example:
                //
                // - KEY_A is the code here
                // - OS layout is AZERTY
                // - No remapping, e.g. active layer is:
                //   - (deflayermap (active-layer) a a)
                //
                // This means kanata received a key press at US-layout position Q. However,
                // translating KEY_A via osc_to_u16 will result in the scancode assocated with
                // US-layout position A, but we want to output the position Q scancode. It is
                // MapVirtualKeyA that does the correct translation for this based on the user's OS
                // layout.
                kb_input.wScan = MapVirtualKeyA(code_u32, 0) as u16;
                if kb_input.wScan == 0 {
                    // The known scenario for this is VK_KPENTER_FAKE which isn't a real VK so
                    // MapVirtualKeyA is expected to return 0. This fake VK is used to
                    // distinguish the key within kanata since in Windows there is no output VK for
                    // the enter key at the numpad.
                    //
                    // The osc_to_u16 function knows the scancode for keypad enter though, and it
                    // isn't known to change based on language layout so this seems fine to do.
                    kb_input.wScan = osc_to_u16(code.into()).unwrap_or(0);
                }
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
            use kanata_parser::keys::*;
            kb_input.wVk = match code {
                VK_KPENTER_FAKE => VK_RETURN as u16,
                _ => code,
            };
        }

        let mut inputs: [INPUT; 1] = mem::zeroed();
        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = kb_input;
        SendInput(1, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as _);
    }
}
