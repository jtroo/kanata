//! A logger that prints to OutputDebugString (Windows only)
use log::{Level, Metadata, Record};

/// Implements `log::Log`, so can be used as a logging provider to
/// forward log messages to the Windows `OutputDebugString` API
pub struct WinDebugLogger;

/// Static instance of `WinDebugLogger`, can be directly registered using `log::set_logger`<br>
/// ```
/// use kanata_passthru::log_win;
/// let _ = log_win::init(); // Init
/// log::set_max_level(log::LevelFilter::Debug);
/// use log::debug; // Use
/// debug!("Debug log");
/// ```
pub static WINDBG_LOGGER: WinDebugLogger = WinDebugLogger;

/// Convert logging levels to shorter and more visible icons
pub fn iconify(lvl: log::Level) -> char {
    match lvl {
        Level::Error => '❗',
        Level::Warn => '⚠',
        Level::Info => 'ⓘ',
        Level::Debug => 'ⓓ',
        Level::Trace => 'ⓣ',
    }
}

use std::sync::OnceLock;
pub fn is_thread_state() -> &'static bool {
    set_thread_state(false)
}
pub fn set_thread_state(is: bool) -> &'static bool {
    // accessor function to avoid get_or_init on every call (lazycell
    // allows doing that without an extra function)
    static CELL: OnceLock<bool> = OnceLock::new();
    CELL.get_or_init(|| is)
}

use lazy_static::lazy_static;
use regex::Regex;
lazy_static! { // shorten source file name, no src/ no .rs ext
  static ref RE_EXT:Regex = Regex::new(r"\..*$"   ).unwrap();
  static ref RE_SRC:Regex = Regex::new(r"src[\\/]").unwrap();
}
fn clean_name(path: Option<&str>) -> String {
    if let Some(p) = path {
        RE_SRC.replace(&RE_EXT.replace(p, ""), "").to_string()
    } else {
        "?".to_string()
    }
}

#[cfg(target_os = "windows")]
use winapi::um::processthreadsapi::GetCurrentThreadId;
impl log::Log for WinDebugLogger {
    #[cfg(windows)]
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    #[cfg(not(windows))]
    fn enabled(&self, metadata: &Metadata) -> bool {
        false
    }
    fn log(&self, record: &Record) {
        #[cfg(not(target_os = "windows"))]
        let thread_id = "";
        #[cfg(target_os = "windows")]
        let thread_id = if *is_thread_state() {
            format!("¦{}¦", unsafe { GetCurrentThreadId() })
        } else {
            "".to_string()
        };
        if self.enabled(record.metadata()) {
            let s = format!(
                "{}{}{}:{} {}",
                thread_id,
                iconify(record.level()),
                clean_name(record.file()),
                record.line().unwrap_or(0),
                record.args()
            );
            dbg_win(&s);
        }
    }
    fn flush(&self) {}
}

pub fn dbg_win(s: &str) {
    //! Calls the `OutputDebugString` API to log a string (on Windows only)<br>
    //! See [`OutputDebugStringW`](https://docs.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-outputdebugstringw).
    #[cfg(windows)]
    {
        let len = s.encode_utf16().count() + 1;
        let mut s_utf16: Vec<u16> = Vec::with_capacity(len + 1);
        s_utf16.extend(s.encode_utf16());
        s_utf16.push(0);
        unsafe {
            OutputDebugStringW(&s_utf16[0]);
        }
    }
}

#[cfg(windows)]
extern "stdcall" {
    fn OutputDebugStringW(chars: *const u16);
}

pub fn init() {
    //! Set `WinDebugLogger` as the active logger<br>
    //! Doesn't panic on failure as it creates other problems for FFI etc.
    match log::set_logger(&WINDBG_LOGGER) {
        Ok(()) => {}
        Err(_) => {
            dbg_win("Warning: ✗ Failed to register WinDebugLogger\n");
        }
    }
}
