use super::*;

use std::sync::LazyLock;

use parking_lot::Mutex;

pub(crate) type SavedClipboardData = HashMap<u16, ClipboardData>;
pub(crate) enum ClipboardData {
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
            Ok(()) => { return; },
            Err(e) => {
                log::error!("error setting clipboard: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

pub(crate) fn clpb_cmd_set(cmd_params: &[String]) {
    todo!()
}

pub(crate) fn clpb_save(id: u16, save_data: &mut SavedClipboardData) {
    for _ in 0..10 {
        match CLIPBOARD.lock().get_text() {
            Ok(cliptext) => {
                save_data.insert(id, Text(cliptext));
            },
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
            },
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
            Text(s) => {
                match CLIPBOARD.lock().set_text(s) {
                    Ok(()) => { return; },
                    Err(e) => e,
                }
            }
            Image(img) => {
                match CLIPBOARD.lock().set_image(img.clone()) {
                    Ok(()) => { return; },
                    Err(e) => e,
                }
            }
        };
        log::error!("error setting clipboard: {e:?}");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}
