use super::*;

#[cfg(not(target_arch = "wasm32"))]
pub use real::*;
#[cfg(not(target_arch = "wasm32"))]
mod real {
    use super::*;
    use std::sync::LazyLock;

    use parking_lot::Mutex;

    pub type SavedClipboardData = HashMap<u16, ClipboardData>;
    #[derive(Debug, Clone)]
    pub enum ClipboardData {
        Text(String),
        Image(arboard::ImageData<'static>),
    }
    use ClipboardData::*;

    static CLIPBOARD: LazyLock<Mutex<arboard::Clipboard>> = LazyLock::new(|| {
        for _ in 0..10 {
            let c = arboard::Clipboard::new();
            if let Ok(goodclip) = c {
                log::trace!("clipboard init");
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
                    log::trace!("clipboard set to {clipboard_string}");
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
        let mut newclip = None;
        for _ in 0..10 {
            match CLIPBOARD.lock().get_text() {
                Ok(cliptext) => {
                    newclip = Some(run_cmd_get_stdout(cmd_and_args, cliptext.as_str()));
                    break;
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
        if let Some(nc) = newclip {
            clpb_set(&nc);
        }
    }

    fn run_cmd_get_stdout(cmd_and_args: &[String], stdin: &str) -> String {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut args = cmd_and_args.iter();
        let executable = args
            .next()
            .expect("parsing should have forbidden empty cmd");
        log::trace!("executing {executable}");
        let mut cmd = Command::new(executable);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        for arg in args {
            log::trace!("arg is {arg}");
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
                    log::trace!("saving to id {id}: {cliptext}");
                    save_data.insert(id, Text(cliptext));
                    return;
                }
                Err(e) => {
                    if matches!(e, arboard::Error::ContentNotAvailable) {
                        // ContentNotAvailable could be an image or missing data
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
                    log::trace!("saving to id {id}: <imgdata>");
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
            log::warn!("tried to set clipboard with missing data in id {id}, doing nothing");
            return;
        };
        for _ in 0..10 {
            let e = match restore_data {
                Text(s) => match CLIPBOARD.lock().set_text(s) {
                    Ok(()) => {
                        log::trace!("restored clipboard with id {id}: {s}");
                        return;
                    }
                    Err(e) => e,
                },
                Image(img) => match CLIPBOARD.lock().set_image(img.clone()) {
                    Ok(()) => {
                        log::trace!("restored clipboard with id {id}: <imgdata>");
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
        log::trace!("setting save id {id} with {content}");
        save_data.insert(id, Text(content.into()));
    }

    #[test]
    fn test_set() {
        let mut sd = SavedClipboardData::default();
        clpb_save_set(1, "hi", &mut sd);
        if let Text(s) = sd.get(&1).unwrap() {
            assert_eq!(s.as_str(), "hi");
        } else {
            panic!("did not expect image data");
        }
        assert!(sd.get(&2).is_none());
    }

    pub(crate) fn clpb_save_cmd_set(
        id: u16,
        cmd_and_args: &[String],
        save_data: &mut SavedClipboardData,
    ) {
        let stdin_content = match save_data.get(&id) {
            Some(slot_data) => match slot_data {
                Text(s) => s.as_str(),
                Image(_) => "",
            },
            None => "",
        };
        let content = run_cmd_get_stdout(cmd_and_args, stdin_content);
        log::trace!("setting save id {id} with {content}");
        save_data.insert(id, Text(content));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_save_cmd_set() {
        let mut sd = SavedClipboardData::default();
        sd.insert(1, Text("one".into()));
        clpb_save_cmd_set(
            1,
            &[
                "powershell.exe".into(),
                "-c".into(),
                "$v = ($Input | Select-Object -First 1); Write-Host -NoNewLine \"$v $v\"".into(),
            ],
            &mut sd,
        );

        if let Text(s) = sd.get(&1).unwrap() {
            assert_eq!("one one", s.as_str());
        } else {
            panic!("did not expect image data");
        }
        assert!(sd.get(&2).is_none());

        clpb_save_cmd_set(
            3,
            &[
                "powershell.exe".into(),
                "-c".into(),
                "Write-Host -NoNewLine 'wat'".into(),
            ],
            &mut sd,
        );
        if let Text(s) = sd.get(&3).unwrap() {
            assert_eq!("wat", s.as_str());
        } else {
            panic!("did not expect image data");
        }
    }

    pub(crate) fn clpb_save_swap(id1: u16, id2: u16, save_data: &mut SavedClipboardData) {
        let data1 = save_data.remove(&id1);
        let data2 = save_data.remove(&id2);
        if let Some(d) = data1 {
            save_data.insert(id2, d);
        }
        if let Some(d) = data2 {
            save_data.insert(id1, d);
        }
    }

    #[test]
    fn test_swap() {
        let mut sd = SavedClipboardData::default();
        sd.insert(1, Text("one".into()));
        sd.insert(2, Text("two".into()));
        clpb_save_swap(1, 2, &mut sd);
        if let Text(s) = sd.get(&1).unwrap() {
            assert_eq!(s.as_str(), "two");
        } else {
            panic!("did not expect image data");
        }
        if let Text(s) = sd.get(&2).unwrap() {
            assert_eq!(s.as_str(), "one");
        } else {
            panic!("did not expect image data");
        }

        sd.insert(3, Text("three".into()));
        clpb_save_swap(3, 4, &mut sd);
        assert!(sd.get(&3).is_none());
        if let Text(s) = sd.get(&4).unwrap() {
            assert_eq!(s.as_str(), "three");
        } else {
            panic!("did not expect image data");
        }

        sd.insert(6, Text("six".into()));
        clpb_save_swap(5, 6, &mut sd);
        if let Text(s) = sd.get(&5).unwrap() {
            assert_eq!(s.as_str(), "six");
        } else {
            panic!("did not expect image data");
        }
        assert!(sd.get(&6).is_none());

        clpb_save_swap(7, 8, &mut sd);
        assert!(sd.get(&7).is_none());
        assert!(sd.get(&8).is_none());
    }
}

#[cfg(target_arch = "wasm32")]
pub use fake::*;
#[cfg(target_arch = "wasm32")]
mod fake {
    #![allow(unused)]
    use super::*;
    pub type SavedClipboardData = HashMap<u16, ClipboardData>;
    #[derive(Debug, Clone)]
    pub enum ClipboardData {
        Text(String),
        Text2(String),
    }

    pub(crate) fn clpb_set(clipboard_string: &str) {}

    pub(crate) fn clpb_cmd_set(cmd_and_args: &[String]) {}

    pub(crate) fn clpb_save(id: u16, save_data: &mut SavedClipboardData) {}

    pub(crate) fn clpb_restore(id: u16, save_data: &SavedClipboardData) {}

    pub(crate) fn clpb_save_set(id: u16, content: &str, save_data: &mut SavedClipboardData) {}

    pub(crate) fn clpb_save_cmd_set(
        id: u16,
        cmd_and_args: &[String],
        save_data: &mut SavedClipboardData,
    ) {
    }

    pub(crate) fn clpb_save_swap(id1: u16, id2: u16, save_data: &mut SavedClipboardData) {}
}
