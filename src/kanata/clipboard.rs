use super::*;

use std::sync::LazyLock;

use parking_lot::Mutex;

pub type SavedClipboardData = HashMap<u16, ClipboardData>;
pub enum ClipboardData {
    Text(String),
    Image(arboard::ImageData<'static>),
}
use ClipboardData::*;

static CLIPBOARD: LazyLock<Mutex<arboard::Clipboard>> = LazyLock::new(|| {
    for _ in 0..10 {
        let c = arboard::Clipboard::new();
        if let Ok(goodclip) = c {
            return Mutex::new(goodclip);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("could not initialize clipboard");
});

pub(crate) fn clpb_set(clipboard_string: &str) {
    for _ in 0..10 {
        match CLIPBOARD.lock().set_text(clipboard_string) {
            Ok(()) => {
                return;
            }
            Err(e) => {
                log::error!("error setting clipboard: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

pub(crate) fn clpb_cmd_set(cmd_and_args: &[String]) {
    for _ in 0..10 {
        match CLIPBOARD.lock().get_text() {
            Ok(cliptext) => {
                let newclip = run_cmd_get_stdout(cmd_and_args, cliptext.as_str());
                clpb_set(&newclip);
            }
            Err(e) => {
                if matches!(e, arboard::Error::ContentNotAvailable) {
                    log::warn!("clipboard is unset or is image data; no-op for cmd-set");
                    return;
                }
                log::error!("error setting clipboard: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn run_cmd_get_stdout(cmd_and_args: &[String], stdin: &str) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut args = cmd_and_args.iter();
    let executable = args
        .next()
        .expect("parsing should have forbidden empty cmd");
    let mut cmd = Command::new(executable);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
    for arg in args {
        cmd.arg(arg);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("failed to spawn cmd, returning empty string for cmd-set: {e:?}");
            return String::new();
        }
    };

    let child_stdin = child.stdin.as_mut().unwrap();
    if let Err(e) = child_stdin.write_all(stdin.as_bytes()) {
        log::warn!("failed to write to stdin: {e:?}");
    }
    child
        .wait_with_output()
        .map(|out| String::from_utf8_lossy(&out.stdout).to_string())
        .unwrap_or_else(|e| {
            log::error!("failed to execute cmd: {e:?}");
            String::new()
        })
}

pub(crate) fn clpb_save(id: u16, save_data: &mut SavedClipboardData) {
    for _ in 0..10 {
        match CLIPBOARD.lock().get_text() {
            Ok(cliptext) => {
                save_data.insert(id, Text(cliptext));
            }
            Err(e) => {
                if matches!(e, arboard::Error::ContentNotAvailable) {
                    break;
                }
                log::error!("error setting clipboard: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    for _ in 0..10 {
        match CLIPBOARD.lock().get_image() {
            Ok(clipimg) => {
                save_data.insert(id, Image(clipimg));
            }
            Err(e) => {
                if matches!(e, arboard::Error::ContentNotAvailable) {
                    break;
                }
                log::error!("error setting clipboard: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

pub(crate) fn clpb_restore(id: u16, save_data: &SavedClipboardData) {
    let Some(restore_data) = save_data.get(&id) else {
        log::warn!("tried to set clipboard with missing data {id}, doing nothing");
        return;
    };
    for _ in 0..10 {
        let e = match restore_data {
            Text(s) => match CLIPBOARD.lock().set_text(s) {
                Ok(()) => {
                    return;
                }
                Err(e) => e,
            },
            Image(img) => match CLIPBOARD.lock().set_image(img.clone()) {
                Ok(()) => {
                    return;
                }
                Err(e) => e,
            },
        };
        log::error!("error setting clipboard: {e:?}");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

pub(crate) fn clpb_save_set(id: u16, content: &str, save_data: &mut SavedClipboardData) {
    save_data.insert(id, Text(content.into()));
}

pub(crate) fn clpb_save_cmd_set(
    id: u16,
    cmd_and_args: &[String],
    save_data: &mut SavedClipboardData,
) {
    let stdin_content = match save_data.get(&id) {
        Some(slot_data) => match slot_data {
            Text(s) => s.as_str(),
            Image(_) => &"",
        },
        None => &"",
    };
    let content = run_cmd_get_stdout(cmd_and_args, stdin_content);
    save_data.insert(id, Text(content.into()));
}

pub(crate) fn clpb_save_swap(id1: u16, id2: u16, save_data: &mut SavedClipboardData) {
    todo!()
}
