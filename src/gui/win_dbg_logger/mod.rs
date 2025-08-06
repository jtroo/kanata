#![allow(non_upper_case_globals)]
//! A logger for use with Windows debuggers.
//!
//! This crate integrates with the ubiquitous [`log`] crate and can be used with the [`simplelog`] crate.
//!
//! Windows allows applications to output a string directly to debuggers. This is very useful in
//! situations where other forms of logging are not available.
//! For example, stderr is not available for GUI apps.
//!
//! Windows provides the `OutputDebugString` entry point, which allows apps to print a debug string.
//! Internally, `OutputDebugString` is implemented by raising an SEH exception, which the debugger
//! catches and handles.
//!
//! Raising an exception has a significant cost, when run under a debugger, because the debugger
//! halts all threads in the target process. So you should avoid using this logger for high rates
//! of output, because doing so will slow down your app.
//!
//! Like many Windows entry points, `OutputDebugString` is actually two entry points:
//! `OutputDebugStringA` (multi-byte encodings) and
//! `OutputDebugStringW` (UTF-16). In most cases, the `*A` version is implemented using a "thunk"
//! which converts its arguments to UTF-16 and then calls the `*W` version. However,
//! `OutputDebugStringA` is one of the few entry points where the opposite is true.
//!
//! This crate can be compiled and used on non-Windows platforms, but it does nothing.
//! This is intended to minimize the impact on code that takes a dependency on this crate.
//!
//! # Example
//!
//! ```rust
//! use log::{debug, info};
//!
//! fn do_cool_stuff() {
//!    info!("Hello, world!");
//!    debug!("Hello, world, in detail!");
//! }
//!
//! fn main() {
//!     log::set_logger(&kanata_state_machine::gui::WINDBG_LOGGER).unwrap();
//!     log::set_max_level(log::LevelFilter::Debug);
//!
//!     do_cool_stuff();
//! }
//! ```

use log::{Level, LevelFilter, Metadata, Record};

/// This implements `log::Log`, and so can be used as a logging provider.
/// It forwards log messages to the Windows `OutputDebugString` API.
#[derive(Copy, Clone)]
pub struct WinDbgLogger {
    level: LevelFilter,
    /// Allow for `WinDbgLogger` to possibly have more fields in the future
    _priv: (),
}

/// This is a static instance of `WinDbgLogger`. Since `WinDbgLogger` contains no state,
/// this can be directly registered using `log::set_logger`, e.g.:
///
/// ```
/// log::set_logger(&kanata_state_machine::gui::WINDBG_LOGGER).unwrap(); // Initialize
/// log::set_max_level(log::LevelFilter::Debug);
///
/// use log::{info, debug}; // Import
///
/// info!("Hello, world!"); debug!("Hello, world, in detail!"); // Use to log
/// ```
pub static WINDBG_LOGGER: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Trace,
    _priv: (),
};
pub static WINDBG_L1: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Error,
    _priv: (),
};
pub static WINDBG_L2: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Warn,
    _priv: (),
};
pub static WINDBG_L3: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Info,
    _priv: (),
};
pub static WINDBG_L4: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Debug,
    _priv: (),
};
pub static WINDBG_L5: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Trace,
    _priv: (),
};
pub static WINDBG_L0: WinDbgLogger = WinDbgLogger {
    level: LevelFilter::Off,
    _priv: (),
};

#[cfg(all(target_os = "windows", feature = "gui"))]
pub fn windbg_simple_combo(
    log_lvl: LevelFilter,
    noti_lvl: LevelFilter,
) -> Box<dyn simplelog::SharedLogger> {
    set_noti_lvl(noti_lvl);
    match log_lvl {
        LevelFilter::Error => Box::new(WINDBG_L1),
        LevelFilter::Warn => Box::new(WINDBG_L2),
        LevelFilter::Info => Box::new(WINDBG_L3),
        LevelFilter::Debug => Box::new(WINDBG_L4),
        LevelFilter::Trace => Box::new(WINDBG_L5),
        LevelFilter::Off => Box::new(WINDBG_L0),
    }
}
#[cfg(all(target_os = "windows", feature = "gui"))]
impl simplelog::SharedLogger for WinDbgLogger {
    // allows using with simplelog's CombinedLogger
    fn level(&self) -> LevelFilter {
        self.level
    }
    fn config(&self) -> Option<&simplelog::Config> {
        None
    }
    fn as_log(self: Box<Self>) -> Box<dyn log::Log> {
        Box::new(*self)
    }
}

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
    // accessor function to avoid get_or_init on every call
    // (lazycell allows doing that without an extra function)
    static CELL: OnceLock<bool> = OnceLock::new();
    CELL.get_or_init(|| is)
}
pub fn get_noti_lvl() -> &'static LevelFilter {
    set_noti_lvl(LevelFilter::Off)
}
pub fn set_noti_lvl(lvl: LevelFilter) -> &'static LevelFilter {
    static CELL: OnceLock<LevelFilter> = OnceLock::new();
    CELL.get_or_init(|| lvl)
}

use regex::Regex;
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}
fn clean_name(path: Option<&str>) -> String {
    let re_ext: &Regex = regex!(r"\..*$"); // shorten source file name, no src/ no .rs ext
    let re_src: &Regex = regex!(r"src[\\/]");
    // remove extension and src paths
    if let Some(p) = path {
        re_src.replace(&re_ext.replace(p, ""), "").to_string()
    } else {
        "?".to_string()
    }
}

#[cfg(target_os = "windows")]
use winapi::um::processthreadsapi::GetCurrentThreadId;
impl log::Log for WinDbgLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        #[cfg(not(target_os = "windows"))]
        let thread_id = "";
        #[cfg(target_os = "windows")]
        let thread_id = if *is_thread_state() {
            format!("{}¦", unsafe { GetCurrentThreadId() })
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
            #[cfg(all(target_os = "windows", feature = "gui"))]
            {
                use crate::gui::win::*;
                let title = format!(
                    "{}{}:{}",
                    thread_id,
                    clean_name(record.file()),
                    record.line().unwrap_or(0)
                );
                let msg = format!("{}", record.args());
                if record.level() <= *get_noti_lvl() {
                    show_err_msg_nofail(title, msg);
                }
            }
            output_debug_string(&s);
        }
    }

    fn flush(&self) {}
}

/// Calls the `OutputDebugString` API to log a string.
///
/// On non-Windows platforms, this function does nothing.
///
/// See [`OutputDebugStringW`](https://docs.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-outputdebugstringw).
pub fn output_debug_string(s: &str) {
    #[cfg(windows)]
    {
        let len = s.encode_utf16().count() + 1;
        let mut s_utf16: Vec<u16> = Vec::with_capacity(len);
        s_utf16.extend(s.encode_utf16());
        s_utf16.push(0);
        unsafe {
            OutputDebugStringW(&s_utf16[0]);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = s;
    }
}

#[cfg(windows)]
unsafe extern "stdcall" {
    fn OutputDebugStringW(chars: *const u16);
    fn IsDebuggerPresent() -> i32;
}

/// Checks whether a debugger is attached to the current process.
///
/// On non-Windows platforms, this function always returns `false`.
///
/// See [`IsDebuggerPresent`](https://docs.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-isdebuggerpresent).
pub fn is_debugger_present() -> bool {
    #[cfg(windows)]
    {
        unsafe { IsDebuggerPresent() != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Sets the `WinDbgLogger` as the currently-active logger.
///
/// If an error occurs when registering `WinDbgLogger` as the current logger, this function will
/// output a warning and will return normally. It will not panic.
/// This behavior was chosen because `WinDbgLogger` is intended for use in debugging.
/// Panicking would disrupt debugging and introduce new failure modes. It would also create
/// problems for mixed-mode debugging, where Rust code is linked with C/C++ code.
pub fn init() {
    match log::set_logger(&WINDBG_LOGGER) {
        Ok(()) => {} //↓ there's really nothing we can do about it.
        Err(_) => {
            output_debug_string(
                "Warning: Failed to register WinDbgLogger as the current Rust logger.\r\n",
            );
        }
    }
}

macro_rules! define_init_at_level {
    ($func:ident, $level:ident) => {
        /// This can be called from C/C++ code to register the debug logger.
        ///
        /// For Windows DLLs that have statically linked an instance of `win_dbg_logger` into
        /// them, `DllMain` should call `win_dbg_logger_init_<level>()` from the `DLL_PROCESS_ATTACH`
        /// handler, e.g.:
        ///
        /// ```ignore
        /// extern "C" void __cdecl rust_win_dbg_logger_init_debug(); // Calls into Rust code
        /// BOOL WINAPI DllMain(HINSTANCE hInstance, DWORD reason, LPVOID reserved) {
        ///   switch (reason) {
        ///     case DLL_PROCESS_ATTACH:
        ///       rust_win_dbg_logger_init_debug();
        ///       // ...
        ///   }
        ///   // ...
        /// }
        /// ```
        ///
        /// For Windows executables that have statically linked an instance of `win_dbg_logger`
        /// into them, call `win_dbg_logger_init_<level>()` during app startup.
        #[unsafe(no_mangle)]
        pub extern "C" fn $func() {
            init();
            log::set_max_level(LevelFilter::$level);
        }
    };
}

define_init_at_level!(rust_win_dbg_logger_init_trace, Trace);
define_init_at_level!(rust_win_dbg_logger_init_info, Info);
define_init_at_level!(rust_win_dbg_logger_init_debug, Debug);
define_init_at_level!(rust_win_dbg_logger_init_warn, Warn);
define_init_at_level!(rust_win_dbg_logger_init_error, Error);
