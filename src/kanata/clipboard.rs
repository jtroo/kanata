use super::*;

use std::sync::LazyLock;

pub(crate) type SavedClipboardData = HashMap<u16, String>;
enum ClipboardData {
    String(String),
    Image(arboard::ImageData<'static>),
}

static CLIPBOARD: LazyLock<arboard::Clipboard> = LazyLock::new(|| {
    for _ in 0..10 {
        let c = arboard::Clipboard::new();
        if let Ok(goodclip) = c {
            return goodclip;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("could not initialize clipboard");
});


pub(crate) fn clpb_set(clipbboard_string: String) {
    todo!()
}

pub(crate) fn clpb_cmd_set(cmd_params: &[String]) {
    todo!()
}

pub(crate) fn clpb_save(id: u16, save_data: &mut SavedClipboardData) {
    todo!()
}

pub(crate) fn clpb_restore(id: u16, save_data: &SavedClipboardData) {
    todo!()
}
