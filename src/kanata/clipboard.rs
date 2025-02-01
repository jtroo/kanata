use super::*;

use std::sync::LazyLock;

pub(crate) type SavedClipboardData = HashMap<u16, String>;

pub(crate) static CLIPBOARD: LazyLock<arboard::Clipboard> = LazyLock::new(|| {
    for _ in 0..10 {
        let c = arboard::Clipboard::new();
        if let Ok(goodclip) = c {
            return goodclip;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("could not initialize clipboard");
});
