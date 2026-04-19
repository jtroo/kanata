//! Contains the input/output code for keyboards on Macos.

// Caused by unmaintained objc crate triggering warnings.
#![allow(unexpected_cfgs)]
#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

use super::*;
use crate::kanata::CalculatedMouseMove;
use crate::oskbd::KeyEvent;
use anyhow::anyhow;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_graphics::base::CGFloat;
use core_graphics::display::{CGDisplay, CGPoint};
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CGMouseButton, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use kanata_parser::cfg::MappedKeys;
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;
use karabiner_driverkit::*;
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::io::Error;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender as Sender;
use std::time::{Duration, Instant};

/// Mouse `OsCode`s that, when present in `MAPPED_KEYS`, justify installing the
/// CGEventTap. Used both as the startup/reload install gate and as the set of
/// codes the tap can produce.
const MOUSE_OSCODES: [OsCode; 9] = [
    OsCode::BTN_LEFT,
    OsCode::BTN_RIGHT,
    OsCode::BTN_MIDDLE,
    OsCode::BTN_SIDE,
    OsCode::BTN_EXTRA,
    OsCode::MouseWheelUp,
    OsCode::MouseWheelDown,
    OsCode::MouseWheelLeft,
    OsCode::MouseWheelRight,
];

/// Sender stashed by the first `start_mouse_listener` call so that
/// `ensure_mouse_listener_installed_after_reload` can install the tap on a
/// later live reload without needing the original `event_loop` context.
static MOUSE_TAP_TX: OnceLock<Sender<KeyEvent>> = OnceLock::new();

/// Tracks whether `start_mouse_listener` has *claimed* the install slot —
/// i.e. promised to spawn a thread that will create and enable a CGEventTap.
/// Claimed via `compare_exchange` *before* `thread::spawn` so a concurrent
/// live reload cannot race in and install a second tap during the brief
/// window before the spawned thread reaches `tap.enable()`. Reset to `false`
/// if `CGEventTap::new` fails, so a future reload (e.g. after the user grants
/// Accessibility permission) can retry.
///
/// Note that "claimed" is slightly stronger than "currently capturing
/// events": there is a sub-millisecond gap between the claim and
/// `tap.enable()` during which no events flow yet. Reload callers
/// short-circuit in that gap, which is correct because the spawned thread
/// will deliver the working tap regardless.
static MOUSE_TAP_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Stashed by the first `start_mouse_listener` call so the CGEventTap callback
/// can read the live `mouse-movement-key` setting on every cursor movement
/// event. The Arc points to the same `parking_lot::Mutex` that the live-reload
/// path updates, so changes take effect with no extra plumbing.
static MOUSE_MOVEMENT_KEY: OnceLock<std::sync::Arc<parking_lot::Mutex<Option<OsCode>>>> =
    OnceLock::new();

// --- Karabiner startup-abort diagnostics ---
//
// On macOS, kanata grabs keyboards via the
// Karabiner-DriverKit-VirtualHIDDevice C++ library (the
// `karabiner-driverkit` crate). That library spawns its own dispatcher
// threads which talk to the `Karabiner-VirtualHIDDevice-Daemon` over
// root-owned IPC files under
// `/Library/Application Support/org.pqrs/tmp/rootonly/`. That directory
// is mode 700 owned by root, so *the kanata process itself must run as
// root* (via `sudo` or a launchd daemon) to reach the sockets inside.
//
// When that invariant is violated — the #1 real-world cause being
// "forgot to `sudo`", followed by "driver not installed / system
// extension not approved" — the C++ dispatcher threads hit an uncaught
// `std::filesystem_error` on a `posix_stat` of the rootonly directory.
// The exception bubbles up on a background thread that has no
// try/catch wrapper, libc++abi calls `std::terminate`, and the process
// aborts via `SIGABRT` with the cryptic message:
//
//     libc++abi: terminating due to uncaught exception of type
//     std::__1::__fs::filesystem::filesystem_error: ...
//
// To turn that into something actionable, we install a `SIGABRT`
// handler that — *after* libc++abi has printed its own message —
// writes a static hint to stderr enumerating the likely causes in
// order (not running as root, driver not approved, exclusive grabber),
// then restores the default handler and re-raises so the abort still
// propagates with the usual exit code / coredump behavior.
//
// The hint is gated on `KARABINER_STARTUP_PHASE`: it is only emitted
// from handler-install time until `mark_karabiner_startup_complete()`
// fires (right after `wait_until_ready` returns on the happy path).
// Any `SIGABRT` after that is almost certainly an unrelated
// dispatcher/CoreFoundation teardown race — for which the Karabiner
// hint would be actively misleading — so the handler silently
// re-raises in that phase. The kill-chord exit path avoids tripping
// the teardown race at all by using `libc::_exit` instead of
// `std::process::exit` (see `check_for_exit` in `src/kanata/mod.rs`).
//
// The handler body uses only async-signal-safe calls
// (`AtomicBool::load`, `write(2)`, `signal`, `raise`).

/// True while kanata is still in the Karabiner startup path — i.e. from
/// the first call to `install_karabiner_abort_handler` until
/// `mark_karabiner_startup_complete()` is called. The `SIGABRT` handler
/// reads this to decide whether to emit the Karabiner hint: during
/// startup, an uncaught exception is almost always a Karabiner setup
/// issue and the hint is actionable; after startup (running normally or
/// tearing down), it's typically a dispatcher/CoreFoundation teardown
/// race and the hint would be misleading.
static KARABINER_STARTUP_PHASE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Signal that kanata has finished talking to the Karabiner daemon during
/// startup and is running normally. After this, the `SIGABRT` handler will
/// stop emitting the Karabiner hint. Idempotent.
pub fn mark_karabiner_startup_complete() {
    KARABINER_STARTUP_PHASE.store(false, std::sync::atomic::Ordering::Release);
}

extern "C" fn karabiner_sigabrt_handler(_sig: libc::c_int) {
    // Only emit the hint during startup — see `KARABINER_STARTUP_PHASE`.
    // `AtomicBool::load` compiles to an async-signal-safe plain load.
    if !KARABINER_STARTUP_PHASE.load(std::sync::atomic::Ordering::Acquire) {
        // Not in startup — restore default handler and re-raise without
        // printing anything extra. The underlying libc++abi message
        // (if any) has already been written to stderr by the time we
        // get here.
        unsafe {
            libc::signal(libc::SIGABRT, libc::SIG_DFL);
            libc::raise(libc::SIGABRT);
        }
        return;
    }
    // Async-signal-safe: only `write(2)` and `signal/raise`. Keep the
    // message as a single static byte string — no formatting, no
    // allocations.
    const HINT: &[u8] = b"\n\
        kanata: aborted while talking to the Karabiner virtual HID daemon.\n\
        The most likely causes, in order:\n\
          1) kanata is not running as root. The Karabiner virtual HID daemon\n\
             exposes its IPC under `/Library/Application Support/org.pqrs/\n\
             tmp/rootonly/`, which only root can access. Re-run kanata with\n\
             `sudo`, or install it as a launchd daemon that runs as root.\n\
          2) Karabiner-DriverKit-VirtualHIDDevice is not installed or its\n\
             system extension has not been approved. Run\n\
             `sudo /Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager forceActivate`,\n\
             approve the driver in System Settings -> General -> Login Items\n\
             & Extensions -> Driver Extensions if prompted, then re-run\n\
             kanata. A reboot may be required after a prior `deactivate`.\n\
          3) Another process is already grabbing your keyboard exclusively.\n\
        \n";
    // SAFETY: write(2) is async-signal-safe and takes a raw fd + buffer.
    unsafe {
        libc::write(
            libc::STDERR_FILENO,
            HINT.as_ptr() as *const libc::c_void,
            HINT.len(),
        );
        // Restore default handler and re-raise so the abort propagates as
        // usual (preserves exit code / coredump behavior).
        libc::signal(libc::SIGABRT, libc::SIG_DFL);
        libc::raise(libc::SIGABRT);
    }
}

// --- Input Monitoring (TCC) pre-flight ---
//
// On macOS, observing raw keyboard events requires the "Input
// Monitoring" TCC permission (System Settings -> Privacy & Security
// -> Input Monitoring). Without it, startup fails deep inside the
// Karabiner DriverKit stack with an error that doesn't mention TCC at
// all, leaving users guessing. Checking up front turns that into a
// single actionable message, and on a first run asks macOS to
// register kanata under Input Monitoring so the user has something to
// toggle on. Flagged in issue #1743.

// IOKit framework bindings for the Input Monitoring TCC gate.
// `IOHIDCheckAccess` reports the current decision; `IOHIDRequestAccess`
// registers the binary under System Settings and (if possible) prompts
// the user. Both take an `IOHIDRequestType` and return an
// `IOHIDAccessType` / `bool`.
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDCheckAccess(request: u32) -> u32;
    fn IOHIDRequestAccess(request: u32) -> bool;
}

// ApplicationServices binding for the Accessibility TCC gate.
// `AXIsProcessTrusted` reports whether the current process is trusted
// for Accessibility without showing any system prompt — exactly what
// we want for a pre-flight diagnostic. The symbol lives in the
// HIServices subframework of ApplicationServices and is available on
// every macOS version kanata supports. core-graphics (already a
// dependency) links CoreGraphics, which transitively loads
// ApplicationServices, but declare the framework link explicitly so
// this does not silently break if that transitive link ever changes.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// `IOHIDRequestType::kIOHIDRequestTypeListenEvent` from
/// `<IOKit/hid/IOHIDLib.h>`: the request type that maps to the Input
/// Monitoring TCC service.
const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
/// `IOHIDAccessType` values from `<IOKit/hid/IOHIDLib.h>`.
const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;
const K_IOHID_ACCESS_TYPE_DENIED: u32 = 1;
const K_IOHID_ACCESS_TYPE_UNKNOWN: u32 = 2;

/// Pre-flight check for Input Monitoring permission. Returns `Ok(())`
/// if kanata is allowed to observe events; otherwise returns an
/// `anyhow::Error` that surfaces as the startup failure reason,
/// pointing the user at the exact setting to flip.
///
/// On the "unknown" (first-run) branch we call `IOHIDRequestAccess`,
/// which adds kanata to System Settings -> Privacy & Security -> Input
/// Monitoring. For a root/LaunchDaemon context that call cannot
/// display a UI prompt and will return false; the returned error
/// message then tells the user where to grant it manually.
fn ensure_input_monitoring_permission() -> Result<(), anyhow::Error> {
    const HINT: &str = "Enable kanata in System Settings -> Privacy & Security -> \
         Input Monitoring, then re-run kanata.";
    // SAFETY: plain FFI call with a scalar arg; the symbol is present
    // on every macOS version kanata supports (10.15+).
    let status = unsafe { IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) };
    match status {
        K_IOHID_ACCESS_TYPE_GRANTED => Ok(()),
        K_IOHID_ACCESS_TYPE_UNKNOWN => {
            log::info!(
                "macOS Input Monitoring permission not yet decided; \
                 asking IOKit to register kanata under System Settings"
            );
            // SAFETY: plain FFI call.
            let granted = unsafe { IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) };
            if granted {
                Ok(())
            } else {
                Err(anyhow!(
                    "kanata needs macOS Input Monitoring permission. {HINT}"
                ))
            }
        }
        K_IOHID_ACCESS_TYPE_DENIED => Err(anyhow!(
            "macOS Input Monitoring permission is denied for kanata. {HINT}"
        )),
        other => {
            log::warn!(
                "IOHIDCheckAccess returned unexpected status {other}; \
                 continuing and letting the driver layer report any failure"
            );
            Ok(())
        }
    }
}

/// Pre-flight check for Accessibility permission. Even after Input
/// Monitoring is granted, grabbing keyboards through the Karabiner
/// DriverKit stack can fail with `IOHIDDeviceOpen error: (iokit/common)
/// not permitted` / `kIOReturnNotPermitted` when the Accessibility TCC
/// service is not granted — issue #1211 and many duplicates. Checking
/// up front turns that into a single actionable message pointing at
/// the exact setting, instead of the generic "grab failed" users see
/// otherwise.
///
/// We deliberately call `AXIsProcessTrusted` (no options) rather than
/// `AXIsProcessTrustedWithOptions` with `kAXTrustedCheckOptionPrompt`:
/// a root LaunchDaemon context cannot display a UI prompt, and for a
/// plain `sudo` invocation the prompt would race kanata's own startup
/// output. The returned error message already tells the user where to
/// grant it manually, mirroring the Input Monitoring branch.
fn ensure_accessibility_permission() -> Result<(), anyhow::Error> {
    // SAFETY: plain FFI call with no args; the symbol is present on
    // every macOS version kanata supports (10.15+).
    if unsafe { AXIsProcessTrusted() } {
        Ok(())
    } else {
        Err(anyhow!(
            "kanata needs macOS Accessibility permission. Enable kanata in \
             System Settings -> Privacy & Security -> Accessibility, then \
             re-run kanata. Note: if you moved, renamed, or upgraded the \
             kanata binary, macOS pins the old path and you must remove the \
             stale entry and re-add the current binary. This is the \
             commonly-missed second permission behind the `IOHIDDeviceOpen \
             error: (iokit/common) not permitted` failure (issue #1211)."
        ))
    }
}

/// Install a `SIGABRT` handler that adds an actionable hint about
/// Karabiner setup issues *after* libc++abi prints its own
/// uncaught-exception message, and enter the "Karabiner startup phase"
/// during which that hint is active. Idempotent and process-global; safe
/// to call multiple times. See the module-level comment for the full
/// rationale.
fn install_karabiner_abort_handler() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    // Enter the startup phase unconditionally — even on a repeat call we
    // want the hint active until `mark_karabiner_startup_complete` runs.
    KARABINER_STARTUP_PHASE.store(true, Ordering::Release);
    if INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    // SAFETY: signal(2) takes an fn pointer with the C ABI; the handler
    // body only calls async-signal-safe functions. The two-step cast goes
    // through `*const ()` to satisfy the `function_casts_as_integer` lint.
    let handler_ptr = karabiner_sigabrt_handler as *const () as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGABRT, handler_ptr);
    }
}

// --- Console session lock / user-switch detection ---
//
// kanata on macOS must run as root, so its keyboard grab is global —
// it stays active at the lock screen and during fast user switching.
// That's hostile to anyone else at the keyboard, who is stuck with
// the kanata user's remap (any layout swap, home-row mods, etc.)
// when trying to type their own password or use their own session.
// Flagged in issue #1743.
//
// Fix: poll `CGSessionCopyCurrentDictionary` from a small background
// thread (~200ms) and toggle `SCREEN_GRAB_PAUSED` when either:
//   - the screen is locked (`CGSSessionScreenIsLocked` boolean —
//     undocumented but stable for many years; empirically the key
//     is *absent* from the dict while unlocked and present-and-true
//     while locked, so missing is treated as unlocked), OR
//   - kanata's session is no longer on the console
//     (`kCGSSessionOnConsoleKey` boolean is false; this one *is*
//     documented in `<CoreGraphics/CGSession.h>`), i.e. another
//     user has taken the console via fast user switching.
//
// Per `<CoreGraphics/CGSession.h>`, `CGSessionCopyCurrentDictionary`
// returns the *caller's* session (not the active console session) or
// NULL if the caller has no Quartz GUI session at all (e.g. root
// LaunchDaemon started at boot before any login). That last case
// matters: NULL must NOT pause, otherwise launchd-daemon installs
// would never grab the keyboard. NULL therefore falls back to "do
// not pause", preserving the historical behavior on that path.
//
// Caveat: `wait_key()` is a blocking `read(2)` on the pqrs pipe, so
// the poller can't wake kanata mid-read. The new user's first
// keystroke after a lock is still seized by IOKit and arrives through
// the pipe; the event loop drops it (see `src/kanata/macos.rs`)
// instead of running it through the layer. Net effect: one lost
// keystroke, never a remapped one.
//
// We do *not* call `release_input_only` from the poller thread to
// wake the read sooner. The C++ side joins the listener thread and
// closes raw FDs without poisoning them, so racing a poller-side
// release against the event-loop's release/regrab could close FDs
// that another kanata thread has reused.

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::CFString;

// Not exposed by `core-graphics`. Symbol lives in `CoreGraphics.framework`
// (re-exported from SkyLight) which `core-graphics` already links.
// Returns a +1-retained dictionary or NULL.
unsafe extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

fn copy_session_dict() -> Option<CFDictionary<CFString, CFType>> {
    // SAFETY: CGSessionCopyCurrentDictionary returns NULL or a +1 retained
    // CFDictionaryRef; wrap_under_create_rule takes ownership of that retain.
    unsafe {
        let raw = CGSessionCopyCurrentDictionary();
        if raw.is_null() {
            None
        } else {
            Some(CFDictionary::wrap_under_create_rule(raw))
        }
    }
}

/// True while kanata's keyboard grab should be paused — currently set
/// when kanata's CGSession reports either `CGSSessionScreenIsLocked`
/// or `kCGSSessionOnConsoleKey == false`.
static SCREEN_GRAB_PAUSED: AtomicBool = AtomicBool::new(false);

/// Opt-in flag, set from `--release-grab-on-lock`. When false (the
/// default), `start_screen_lock_poller` is a no-op and
/// `is_screen_grab_paused` always returns false, preserving the
/// historical always-grab behavior for users who run kanata on a
/// single-user Mac and want the remap active even at the lock screen.
static RELEASE_GRAB_ON_LOCK_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_release_grab_on_lock(enabled: bool) {
    RELEASE_GRAB_ON_LOCK_ENABLED.store(enabled, Ordering::Release);
}

pub fn is_screen_grab_paused() -> bool {
    SCREEN_GRAB_PAUSED.load(Ordering::Acquire)
}

/// Look up a `CFBoolean`-valued key on the given session dictionary.
/// Returns `None` if the key is missing or the value is the wrong type.
fn dict_bool(dict: &CFDictionary<CFString, CFType>, key: &'static str) -> Option<bool> {
    let key = CFString::from_static_string(key);
    let value = dict.find(&key)?;
    value.downcast::<CFBoolean>().map(bool::from)
}

/// Decide whether the keyboard grab should currently be paused, based
/// on the live CGSession state for kanata's own session.
fn should_pause_for_session() -> bool {
    // No GUI session (launchd root daemon at boot) — preserve the
    // historical "always grab" behavior. See module-level notes.
    let Some(dict) = copy_session_dict() else {
        return false;
    };
    if dict_bool(&dict, "CGSSessionScreenIsLocked").unwrap_or(false) {
        return true;
    }
    // Missing OnConsole key (early-boot / loginwindow) → treat as
    // still-on-console to avoid false pauses.
    !dict_bool(&dict, "kCGSSessionOnConsoleKey").unwrap_or(true)
}

/// Spawn the screen-lock / user-switch poller thread once. The thread
/// runs for the process lifetime, polling every 200ms. Idempotent.
/// No-op unless `--release-grab-on-lock` was passed on the CLI.
pub fn start_screen_lock_poller() {
    if !RELEASE_GRAB_ON_LOCK_ENABLED.load(Ordering::Acquire) {
        return;
    }
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    // Seed the flag synchronously before any read happens, so a
    // kanata started while the screen is already locked won't get a
    // free 200ms grab window before the first poll iteration.
    let initial_paused = should_pause_for_session();
    SCREEN_GRAB_PAUSED.store(initial_paused, Ordering::Release);
    log::info!(
        "screen-lock poller: starting (initial state: {})",
        if initial_paused { "paused" } else { "active" },
    );

    if let Err(e) = std::thread::Builder::new()
        .name("screen-lock-poller".into())
        .spawn(move || {
            let mut last_paused = initial_paused;
            loop {
                let now_paused = should_pause_for_session();
                if now_paused != last_paused {
                    SCREEN_GRAB_PAUSED.store(now_paused, Ordering::Release);
                    if now_paused {
                        log::info!(
                            "screen lock or user-switch detected — keyboard grab will pause on next event"
                        );
                    } else {
                        log::info!(
                            "console session restored — keyboard grab will resume"
                        );
                    }
                    last_paused = now_paused;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        })
    {
        log::warn!("failed to spawn screen-lock poller thread: {e}");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub value: u64,
    pub page: u32,
    pub code: u32,
}

impl InputEvent {
    pub fn new(event: DKEvent) -> Self {
        InputEvent {
            value: event.value,
            page: event.page,
            code: event.code,
        }
    }
}

impl From<InputEvent> for DKEvent {
    fn from(event: InputEvent) -> Self {
        Self {
            value: event.value,
            page: event.page,
            code: event.code,
            device_hash: 0,
        }
    }
}

pub struct KbdIn {
    grabbed: bool,
}

impl Drop for KbdIn {
    fn drop(&mut self) {
        if self.grabbed {
            release();
        }
    }
}

impl KbdIn {
    pub fn new(
        include_names: Option<Vec<String>>,
        exclude_names: Option<Vec<String>>,
    ) -> Result<Self, anyhow::Error> {
        // Pre-flight the Input Monitoring TCC gate before touching the
        // Karabiner stack. A denied permission here produces a clean,
        // actionable error pointing the user at the exact System
        // Settings pane, instead of a confusing libc++abi abort from
        // the driverkit dispatcher threads. See the "Input Monitoring
        // (TCC) pre-flight" block above for the full rationale.
        ensure_input_monitoring_permission()?;

        // Pre-flight the Accessibility TCC gate too. Even with Input
        // Monitoring granted, `IOHIDDeviceOpen` inside the Karabiner
        // stack returns `kIOReturnNotPermitted` ("(iokit/common) not
        // permitted") when Accessibility is missing — issue #1211 and
        // its many duplicates. Catching that here gives the user one
        // clean error pointing at the right pane instead of a cryptic
        // `grab failed` after a noisy IOKit log line.
        ensure_accessibility_permission()?;

        // Install the SIGABRT hint handler before touching the
        // karabiner-driverkit C++ code, so any uncaught
        // `std::filesystem_error` from the C++ dispatcher threads gets
        // decorated with an actionable Karabiner-setup message after
        // libc++abi's output. See the module-level comment block for the
        // full rationale (tl;dr: most commonly "kanata is not running as
        // root").
        install_karabiner_abort_handler();

        if !driver_activated() {
            return Err(anyhow!(
                "Karabiner-VirtualHIDDevice driver is not activated."
            ));
        }

        // Based on the definition of include and exclude names, they should never be used together.
        // Kanata config parser should probably enforce this.
        let device_names = if let Some(included_names) = include_names {
            validate_and_register_devices(included_names)
        } else {
            // No include list: enumerate every device the driverkit iterator
            // sees, drop any that are known-problematic (empty names, Sidecar
            // virtual keyboards, etc., see `is_skipped_virtual_device`), then
            // apply the user's exclude list on top. This replaces the former
            // `register_device("")` catch-all, which silently seized Sidecar's
            // virtual HID device and could abort the process during grab
            // (issue #1342).
            let excluded = exclude_names.unwrap_or_default();
            let kb_list = fetch_devices();
            let devices_to_include = kb_list
                .iter()
                .filter(|k| !excluded.iter().any(|n| *k == n.as_str()))
                .filter(|k| !is_skipped_virtual_device(&k.product_key))
                .map(|k| {
                    if k.product_key.trim().is_empty() {
                        format!("{:x}", k.hash)
                    } else {
                        k.product_key.clone()
                    }
                })
                .collect::<Vec<String>>();

            validate_and_register_devices(devices_to_include)
        };

        if !device_names.is_empty() {
            if grab() {
                Ok(Self { grabbed: true })
            } else {
                // We have already pre-flighted Input Monitoring and
                // Accessibility, so by this point the most common
                // remaining cause of a `grab failed` is a stale TCC
                // entry that still points at an older copy of the
                // kanata binary — macOS pins the granted path, and
                // after a move/rename/upgrade the new binary is not
                // actually trusted even though the UI shows an entry.
                // See issue #1211.
                Err(anyhow!(
                    "grab failed. kanata could not open the keyboard device \
                     despite Input Monitoring and Accessibility being \
                     reported as granted. If you recently moved, renamed, \
                     or upgraded the kanata binary, remove kanata from \
                     System Settings -> Privacy & Security -> Input \
                     Monitoring *and* Accessibility, then re-add the \
                     current binary (macOS pins TCC grants to the original \
                     path). Also verify kanata is running as root (via \
                     sudo or a LaunchDaemon) and that no other process is \
                     exclusively grabbing the keyboard."
                ))
            }
        } else {
            Err(anyhow!(
                "Couldn't register any device. Use 'kanata --list' to see available devices. \
                 Note: devices with empty names and known virtual devices (e.g. Sidecar) are \
                 automatically skipped to prevent crashes."
            ))
        }
    }

    pub fn read(&mut self) -> Result<InputEvent, io::Error> {
        let mut event = DKEvent {
            value: 0,
            page: 0,
            code: 0,
            device_hash: 0,
        };

        let got_event = wait_key(&mut event);
        if got_event == 0 {
            // Pipe returned EOF — input was released via release_input_only()
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "input pipe closed (devices released)",
            ));
        }

        Ok(InputEvent::new(event))
    }

    /// Release seized input devices without tearing down the output connection.
    /// After this call, `read()` will return `UnexpectedEof`.
    pub fn release_input(&mut self) {
        if self.grabbed {
            release_input_only();
            self.grabbed = false;
        }
    }

    /// Re-seize input devices after a previous `release_input()`.
    /// Returns true if at least one device was seized.
    pub fn regrab_input(&mut self) -> bool {
        if !self.grabbed {
            let ok = karabiner_driverkit::regrab_input();
            self.grabbed = ok;
            ok
        } else {
            true
        }
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }
}

/// Device product-name patterns to skip in the default (no explicit
/// include list) enumeration path. These are virtual HID devices that
/// appear in the keyboard iterator but cannot or must not be seized:
/// seizing them either aborts the process (Sidecar, see issue #1342)
/// or is simply noise the user cannot have intended.
///
/// Matched case-insensitively against the device's product name. Users
/// who need to keep one of these can add it to `macos-dev-names-include`
/// explicitly; that path bypasses this filter.
const SKIPPED_VIRTUAL_DEVICE_SUBSTRINGS: &[&str] = &[
    // Apple Sidecar: iPad-as-display exposes a virtual HID keyboard.
    // Seizing it has aborted kanata during grab() on multiple reporters.
    "sidecar",
    // Karabiner's own virtual keyboard. The driverkit layer already
    // refuses to seize it, but skipping it earlier avoids a misleading
    // "couldn't register" warning in the common list.
    "karabiner",
];

fn is_skipped_virtual_device(product_key: &str) -> bool {
    let lower = product_key.to_lowercase();
    SKIPPED_VIRTUAL_DEVICE_SUBSTRINGS
        .iter()
        .any(|needle| lower.contains(needle))
}

fn validate_and_register_devices(include_names: Vec<String>) -> Vec<String> {
    include_names
        .iter()
        .filter_map(|dev| {
            // Defensive check: skip empty device names that could cause crashes
            if dev.trim().is_empty() {
                log::warn!("Skipping empty device name (likely old keyboard without proper identification)");
                return None;
            }

            // Also skip the Karabiner device
            // driverkit already prevents registering it, but this avoids unnecessary warnings
            if dev.to_lowercase().contains("karabiner") {
                return None;
            }

            match device_matches(dev) {
                true => Some(dev.to_string()),
                false => {
                    log::warn!("'{dev}' doesn't match any connected device");
                    None
                }
            }
        })
        .filter_map(|dev| {
            if register_device(&dev) {
                Some(dev.to_string())
            } else {
                log::warn!("Couldn't register device '{}' - device may be in use by another application or disconnected", dev);
                None
            }
        })
        .collect()
}

impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use kanata_keyberon::key_code::KeyCode;
        match KeyEvent::try_from(*self) {
            Ok(ke) => {
                let direction = match ke.value {
                    KeyValue::Press => "↓",
                    KeyValue::Release => "↑",
                    KeyValue::Repeat => "⟳",
                    KeyValue::Tap => "↕",
                    KeyValue::WakeUp => "!",
                };
                let key_name = KeyCode::from(ke.code);
                write!(f, "{direction}{key_name:?}")
            }
            Err(()) => {
                write!(
                    f,
                    "?unknown(page=0x{:02X},code=0x{:02X})",
                    self.page, self.code
                )
            }
        }
    }
}

impl TryFrom<InputEvent> for KeyEvent {
    type Error = ();

    fn try_from(item: InputEvent) -> Result<Self, Self::Error> {
        if let Ok(oscode) = OsCode::try_from(PageCode {
            page: item.page,
            code: item.code,
        }) {
            Ok(KeyEvent {
                code: oscode,
                value: if item.value == 1 {
                    KeyValue::Press
                } else {
                    KeyValue::Release
                },
            })
        } else {
            Err(())
        }
    }
}

impl TryFrom<KeyEvent> for InputEvent {
    type Error = ();

    fn try_from(item: KeyEvent) -> Result<Self, Self::Error> {
        if let Ok(pagecode) = PageCode::try_from(item.code) {
            let val = match item.value {
                KeyValue::Press | KeyValue::Repeat => 1,
                _ => 0,
            };
            Ok(InputEvent {
                value: val,
                page: pagecode.page,
                code: pagecode.code,
            })
        } else {
            Err(())
        }
    }
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
pub struct KbdOut {
    output_pressed_since: HashMap<OsCode, Instant>,
}

/// Treat a sink-disconnect from the processing thread as a non-fatal drop.
///
/// The macOS event loop (`src/kanata/macos.rs`) coordinates recovery when the
/// DriverKit sink goes away (e.g. on wake-from-sleep) by polling
/// `output_ready()` and re-grabbing input. The processing thread runs in
/// parallel and can race ahead, attempting a write before the event loop
/// notices. Without this, that write would propagate `NotConnected` up to
/// `handle_keys` and panic the processing loop.
#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
fn drop_if_sink_disconnected(
    err: io::Error,
    key: OsCode,
    value: KeyValue,
) -> Result<(), io::Error> {
    if err.kind() == io::ErrorKind::NotConnected {
        log::warn!("dropping {key:?} {value:?}: output backend unavailable (will recover)");
        Ok(())
    } else {
        Err(err)
    }
}

#[cfg(all(not(feature = "simulated_output"), not(feature = "passthru_ahk")))]
impl KbdOut {
    pub fn new() -> Result<Self, io::Error> {
        Ok(KbdOut {
            output_pressed_since: HashMap::default(),
        })
    }

    pub fn write(&mut self, event: InputEvent) -> Result<(), io::Error> {
        let mut devent = event.into();
        log::debug!("Attempting to write {event:?} {devent:?}");
        let rc = send_key(&mut devent);
        if rc == 2 {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "DriverKit virtual keyboard not ready (sink disconnected)",
            ));
        }
        Ok(())
    }

    pub fn output_ready(&self) -> bool {
        is_sink_ready()
    }

    pub fn wait_until_ready(&self, timeout: Option<Duration>) -> bool {
        let start = Instant::now();
        let mut attempt = 0u32;

        loop {
            if self.output_ready() {
                return true;
            }

            if let Some(timeout) = timeout
                && start.elapsed() >= timeout
            {
                return false;
            }

            attempt += 1;
            if attempt % 10 == 0 {
                if let Some(timeout) = timeout {
                    log::info!(
                        "Waiting for DriverKit virtual keyboard... ({:.1}s/{:.1}s)",
                        start.elapsed().as_secs_f64(),
                        timeout.as_secs_f64()
                    );
                } else {
                    log::info!(
                        "Waiting for DriverKit virtual keyboard... ({:.1}s)",
                        start.elapsed().as_secs_f64()
                    );
                }
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn write_key(&mut self, key: OsCode, value: KeyValue) -> Result<(), io::Error> {
        if let Ok(event) = InputEvent::try_from(KeyEvent { value, code: key }) {
            match self.write(event) {
                Ok(()) => {
                    self.record_output_transition_after_write(key, value);
                    Ok(())
                }
                Err(e) => drop_if_sink_disconnected(e, key, value),
            }
        } else {
            log::debug!("couldn't write unrecognized {key:?}");
            Err(io::Error::other("OsCode not recognized!"))
        }
    }

    pub fn write_code(&mut self, code: u32, value: KeyValue) -> Result<(), io::Error> {
        let Some(key) = OsCode::from_u16(code as u16) else {
            log::debug!("couldn't write unrecognized OsCode {code}");
            return Err(io::Error::other("OsCode not recognized!"));
        };
        if let Ok(event) = InputEvent::try_from(KeyEvent { value, code: key }) {
            match self.write(event) {
                Ok(()) => Ok(()),
                Err(e) => drop_if_sink_disconnected(e, key, value),
            }
        } else {
            log::debug!("couldn't write unrecognized OsCode {code}");
            Err(io::Error::other("OsCode not recognized!"))
        }
    }

    pub fn press_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Press)
    }

    pub fn release_key(&mut self, key: OsCode) -> Result<(), io::Error> {
        self.write_key(key, KeyValue::Release)
    }

    pub fn release_tracked_output_keys(&mut self, reason: &str) {
        let tracked_keys: Vec<OsCode> = self.output_pressed_since.keys().copied().collect();
        if tracked_keys.is_empty() {
            return;
        }

        for key in tracked_keys {
            if let Err(error) = self.write_key(key, KeyValue::Release) {
                log::warn!(
                    "failed to release tracked output key during {} recovery: key={key:?} error={error}",
                    reason
                );
            }
        }

        self.output_pressed_since.clear();
    }

    pub fn send_unicode(&mut self, c: char) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        let mut arr = [0u16; 2];
        // Capture the slice containing the encoded UTF-16 code units.
        let encoded = c.encode_utf16(&mut arr);
        // Pass only the part of the array that was populated.
        event.set_string_from_utf16_unchecked(encoded);
        event.set_type(CGEventType::KeyDown);
        event.post(CGEventTapLocation::AnnotatedSession);
        event.set_type(CGEventType::KeyUp);
        event.post(CGEventTapLocation::AnnotatedSession);
        Ok(())
    }
    pub fn scroll(&mut self, direction: MWheelDirection, distance: u16) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        event.set_type(CGEventType::ScrollWheel);
        match direction {
            MWheelDirection::Down => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1,
                distance as i64,
            ),
            MWheelDirection::Up => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1,
                -(distance as i64),
            ),
            MWheelDirection::Left => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
                distance as i64,
            ),
            MWheelDirection::Right => event.set_integer_value_field(
                EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2,
                -(distance as i64),
            ),
        }
        // Mouse control only seems to work with CGEventTapLocation::HID.
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
    /// Synthesize a mouse button press or release via CGEvent.
    ///
    /// Side buttons (Backward/Forward) use OtherMouseDown/Up with
    /// CGMouseButton::Center as a placeholder, then override the
    /// MOUSE_EVENT_BUTTON_NUMBER field to the real index (3=Back, 4=Forward).
    /// The Rust CGMouseButton enum only has 3 variants but the underlying
    /// Apple API supports up to 32 buttons via this field.
    ///
    /// Ref: [init(mouseEventSource:mouseType:mouseCursorPosition:mouseButton:)][1], [setIntegerValueField][2]
    ///
    /// [1]: https://developer.apple.com/documentation/coregraphics/cgevent/init(mouseeventsource:mousetype:mousecursorposition:mousebutton:)
    /// [2]: https://developer.apple.com/documentation/coregraphics/cgevent/setintegervaluefield(_:value:)
    fn button_action(&mut self, btn: Btn, is_click: bool) -> Result<(), io::Error> {
        // (event_type, placeholder_button, real_button_number_override)
        let (event_type, button, button_number) = match btn {
            Btn::Left => (
                if is_click {
                    CGEventType::LeftMouseDown
                } else {
                    CGEventType::LeftMouseUp
                },
                CGMouseButton::Left,
                None,
            ),
            Btn::Right => (
                if is_click {
                    CGEventType::RightMouseDown
                } else {
                    CGEventType::RightMouseUp
                },
                CGMouseButton::Right,
                None,
            ),
            Btn::Mid => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                CGMouseButton::Center,
                None,
            ),
            // Side buttons use OtherMouseDown/Up (same event type as middle click)
            // with the button number overridden after event creation.
            Btn::Backward => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                CGMouseButton::Center,
                Some(3), // USB HID button 4 -> CGEvent button 3 (0-indexed)
            ),
            Btn::Forward => (
                if is_click {
                    CGEventType::OtherMouseDown
                } else {
                    CGEventType::OtherMouseUp
                },
                CGMouseButton::Center,
                Some(4), // USB HID button 5 -> CGEvent button 4 (0-indexed)
            ),
        };

        let event_source = Self::make_event_source()?;
        let event = Self::make_event()?;
        let mouse_position = event.location();
        let event = CGEvent::new_mouse_event(event_source, event_type, mouse_position, button)
            .map_err(|_| std::io::Error::other("Failed to create mouse event"))?;

        if let Some(num) = button_number {
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, num);
        }

        // Mouse control only seems to work with CGEventTapLocation::HID.
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    pub fn click_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        Self::button_action(self, btn, true)
    }

    pub fn release_btn(&mut self, btn: Btn) -> Result<(), io::Error> {
        Self::button_action(self, btn, false)
    }

    pub fn move_mouse(&mut self, mv: CalculatedMouseMove) -> Result<(), io::Error> {
        let pressed = Self::pressed_buttons();

        let event_type = if pressed & 1 > 0 {
            CGEventType::LeftMouseDragged
        } else if pressed & 2 > 0 {
            CGEventType::RightMouseDragged
        } else {
            CGEventType::MouseMoved
        };

        let event = Self::make_event()?;
        let mut mouse_position = event.location();
        Self::apply_calculated_move(&mv, &mut mouse_position);
        if let Ok(event) = CGEvent::new_mouse_event(
            Self::make_event_source()?,
            event_type,
            mouse_position,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
        }
        Ok(())
    }

    fn pressed_buttons() -> usize {
        if let Some(ns_event) = Class::get("NSEvent") {
            unsafe { msg_send![ns_event, pressedMouseButtons] }
        } else {
            0
        }
    }

    pub fn move_mouse_many(&mut self, moves: &[CalculatedMouseMove]) -> Result<(), io::Error> {
        let event = Self::make_event()?;
        let mut mouse_position = event.location();
        let display = CGDisplay::main();
        for current_move in moves.iter() {
            Self::apply_calculated_move(current_move, &mut mouse_position);
        }
        display
            .move_cursor_to_point(mouse_position)
            .map_err(|_| io::Error::other("failed to move mouse"))?;
        Ok(())
    }

    pub fn set_mouse(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        let display = CGDisplay::main();
        let point = CGPoint::new(x as CGFloat, y as CGFloat);
        display
            .move_cursor_to_point(point)
            .map_err(|_| io::Error::other("failed to move cursor to point"))?;
        Ok(())
    }

    fn make_event_source() -> Result<CGEventSource, Error> {
        CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| Error::other("failed to create core graphics event source"))
    }
    /// Creates a core graphics event.
    /// `CombinedSessionState` merges state from all event sources in the
    /// current login session, which is what a remapper needs.
    /// Note that the CFRelease function mentioned in the docs is automatically called when the
    /// event is dropped, therefore we don't need to care about this ourselves.
    fn make_event() -> Result<CGEvent, Error> {
        let event_source = Self::make_event_source()?;
        let event = CGEvent::new(event_source)
            .map_err(|_| Error::other("failed to create core graphics event"))?;
        Ok(event)
    }

    fn record_output_transition_after_write(&mut self, key: OsCode, value: KeyValue) {
        match value {
            KeyValue::Press | KeyValue::Repeat => {
                self.output_pressed_since
                    .entry(key)
                    .or_insert_with(Instant::now);
            }
            KeyValue::Release => {
                self.output_pressed_since.remove(&key);
            }
            KeyValue::Tap | KeyValue::WakeUp => {}
        }
    }

    /// Applies a calculated mouse move to a CGPoint.
    ///
    /// This does _not_ move the mouse, it just mutates the point.
    fn apply_calculated_move(mv: &CalculatedMouseMove, mouse_position: &mut CGPoint) {
        match mv.direction {
            MoveDirection::Up => mouse_position.y -= mv.distance as CGFloat,
            MoveDirection::Down => mouse_position.y += mv.distance as CGFloat,
            MoveDirection::Left => mouse_position.x -= mv.distance as CGFloat,
            MoveDirection::Right => mouse_position.x += mv.distance as CGFloat,
        }
    }
}

/// Convert a `(CGEventType, button_number)` pair from a CGEventTap into a
/// kanata `KeyEvent`. The button number field is only meaningful for
/// `OtherMouseDown`/`OtherMouseUp` (2=Middle, 3=Back, 4=Forward); Left/Right
/// are determined entirely by the event type.
impl TryFrom<(CGEventType, i64)> for KeyEvent {
    type Error = ();
    fn try_from((event_type, button_number): (CGEventType, i64)) -> Result<Self, ()> {
        use OsCode::*;
        let (code, value) = match event_type {
            CGEventType::LeftMouseDown => (BTN_LEFT, KeyValue::Press),
            CGEventType::LeftMouseUp => (BTN_LEFT, KeyValue::Release),
            CGEventType::RightMouseDown => (BTN_RIGHT, KeyValue::Press),
            CGEventType::RightMouseUp => (BTN_RIGHT, KeyValue::Release),
            CGEventType::OtherMouseDown | CGEventType::OtherMouseUp => {
                let code = match button_number {
                    2 => BTN_MIDDLE,
                    3 => BTN_SIDE,
                    4 => BTN_EXTRA,
                    _ => return Err(()),
                };
                let value = if matches!(event_type, CGEventType::OtherMouseDown) {
                    KeyValue::Press
                } else {
                    KeyValue::Release
                };
                (code, value)
            }
            _ => return Err(()),
        };
        Ok(KeyEvent { code, value })
    }
}

/// Decode a `ScrollWheel` `CGEvent` into a kanata `KeyEvent`. A scroll event
/// may carry both axes simultaneously (diagonal scroll on a trackpad); we
/// pick the dominant axis with vertical winning ties, matching how Linux
/// processes one `REL_WHEEL`/`REL_HWHEEL` at a time. The axis/sign convention
/// mirrors `OsKbdOut::scroll`.
fn scroll_event_to_key_event(event: &CGEvent) -> Option<KeyEvent> {
    use OsCode::*;
    let dy = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
    let dx = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
    let code = if dy.abs() >= dx.abs() {
        match dy.signum() {
            1 => MouseWheelDown,
            -1 => MouseWheelUp,
            _ => return None,
        }
    } else {
        match dx.signum() {
            1 => MouseWheelLeft,
            -1 => MouseWheelRight,
            _ => return None,
        }
    };
    Some(KeyEvent {
        code,
        value: KeyValue::Tap,
    })
}

/// Start a CGEventTap on a background thread to intercept mouse button events
/// and (optionally) cursor movement events. macOS equivalent of the Windows
/// mouse hook in `windows/llhook.rs` plus the cursor-movement branch of the
/// Linux event loop.
///
/// Mapped buttons are suppressed and forwarded to the processing channel;
/// unmapped buttons pass through. If `mouse_movement_key` is `Some`, every
/// cursor movement (including drags) sends a synthetic `Tap` of the configured
/// `OsCode` on the channel without suppressing the underlying movement event.
///
/// Only installed if the config has mouse buttons in defsrc OR
/// `mouse-movement-key` is configured.
///
/// Requires Accessibility or Input Monitoring permission.
pub fn start_mouse_listener(
    tx: Sender<KeyEvent>,
    mapped_keys: &MappedKeys,
    mouse_movement_key: std::sync::Arc<parking_lot::Mutex<Option<OsCode>>>,
) -> Option<std::thread::JoinHandle<()>> {
    // Stash both unconditionally so the reload helper always has them, even
    // if this initial call bails on the install gate. `OnceLock::set` is a
    // no-op on subsequent calls — we rely on the single-process,
    // single-Kanata assumption: the inner `parking_lot::Mutex` is shared with
    // `do_live_reload`, so reloads mutate the *value*, never replace the
    // Arc. The `debug_assert!` surfaces accidental violations in test builds.
    let tx_was_unset = MOUSE_TAP_TX.set(tx.clone()).is_ok();
    let _ = MOUSE_MOVEMENT_KEY.set(mouse_movement_key.clone());
    debug_assert!(
        tx_was_unset
            || std::sync::Arc::ptr_eq(
                MOUSE_MOVEMENT_KEY
                    .get()
                    .expect("set above or already present"),
                &mouse_movement_key,
            ),
        "start_mouse_listener called twice with a different mouse_movement_key Arc — \
         the previously stashed Arc would be silently kept"
    );

    let has_mouse_keys = MOUSE_OSCODES.iter().any(|c| mapped_keys.contains(c));
    let has_movement_key = mouse_movement_key.lock().is_some();
    if !has_mouse_keys && !has_movement_key {
        log::info!(
            "No mouse buttons/wheel in defsrc and no mouse-movement-key configured. \
             Not installing mouse event tap."
        );
        return None;
    }

    // Claim the install slot atomically *before* spawning. Closes the race
    // where a live reload could observe `MOUSE_TAP_INSTALLED == false` between
    // the spawn here and the spawned thread's `tap.enable()`, and try to
    // install a second tap. If the claim fails, an installation is already in
    // progress (or completed) — the running tap reads both globals live, so
    // this caller has nothing to do.
    if MOUSE_TAP_INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }

    let spawn_result = std::thread::Builder::new()
        .name("mouse-event-tap".into())
        .spawn(move || {
            let events_of_interest = vec![
                CGEventType::LeftMouseDown,
                CGEventType::LeftMouseUp,
                CGEventType::RightMouseDown,
                CGEventType::RightMouseUp,
                CGEventType::OtherMouseDown,
                CGEventType::OtherMouseUp,
                CGEventType::ScrollWheel,
                CGEventType::MouseMoved,
                CGEventType::LeftMouseDragged,
                CGEventType::RightMouseDragged,
                CGEventType::OtherMouseDragged,
            ];

            // Tap at the *session* location, not HID. HID taps sit upstream of
            // the window server and coalesce with raw HID delivery; on some
            // Bluetooth mice this either freezes the cursor entirely
            // (issue #739) or pins it to the screen edges because relative
            // movement deltas race with the tap thread (issue #1636, Logitech
            // M720 and friends). Session taps sit downstream of the window
            // server and see post-acceleration button/wheel/movement events,
            // which is everything kanata actually needs and does not interfere
            // with cursor delivery. Note: *posting* synthetic mouse events
            // still uses the HID location; that's unrelated and intentional.
            let tap = match CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                events_of_interest,
                // Callback receives &CGEvent; return Some(clone) to pass through,
                // None to suppress the event.
                move |_proxy, event_type, event| {
                    // Cursor movement (incl. drags while a button is held).
                    // Always pass through — never suppress, or the cursor freezes.
                    if matches!(
                        event_type,
                        CGEventType::MouseMoved
                            | CGEventType::LeftMouseDragged
                            | CGEventType::RightMouseDragged
                            | CGEventType::OtherMouseDragged
                    ) {
                        // The Arc is stashed before this tap is created, so
                        // `get()` is `Some` in practice. Fall back to a plain
                        // pass-through if not, rather than panicking on the
                        // hot path.
                        let mmk_slot = match MOUSE_MOVEMENT_KEY.get() {
                            Some(slot) => slot,
                            None => return Some(event.clone()),
                        };
                        if let Some(code) = *mmk_slot.lock() {
                            let fake = KeyEvent {
                                code,
                                value: KeyValue::Tap,
                            };
                            if let Err(e) = tx.try_send(fake) {
                                // Drops are expected under high movement rates;
                                // the user only needs one tap to refresh their
                                // hold timer, so this is not user-visible.
                                log::trace!("mouse tap (movement): drop synthetic tap: {e}");
                            }
                        }
                        return Some(event.clone());
                    }

                    if matches!(event_type, CGEventType::ScrollWheel) {
                        let Some(key_event) = scroll_event_to_key_event(event) else {
                            return Some(event.clone());
                        };
                        if !crate::kanata::MAPPED_KEYS.lock().contains(&key_event.code) {
                            return Some(event.clone());
                        }
                        log::debug!("mouse tap (wheel): {key_event:?}");
                        if let Err(e) = tx.try_send(key_event) {
                            log::warn!("mouse tap: failed to send wheel event: {e}");
                            return Some(event.clone());
                        }
                        return None;
                    }

                    let button_number =
                        event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER);
                    let mut key_event = match KeyEvent::try_from((event_type, button_number)) {
                        Ok(ev) => ev,
                        Err(()) => return Some(event.clone()),
                    };

                    if !crate::kanata::MAPPED_KEYS.lock().contains(&key_event.code) {
                        return Some(event.clone());
                    }

                    // Track pressed state to convert duplicate presses into repeats,
                    // matching the keyboard event loop behavior.
                    match key_event.value {
                        KeyValue::Release => {
                            crate::kanata::PRESSED_KEYS.lock().remove(&key_event.code);
                        }
                        KeyValue::Press => {
                            let mut pressed_keys = crate::kanata::PRESSED_KEYS.lock();
                            if pressed_keys.contains(&key_event.code) {
                                key_event.value = KeyValue::Repeat;
                            } else {
                                pressed_keys.insert(key_event.code);
                            }
                        }
                        _ => {}
                    }

                    log::debug!("mouse tap: {key_event:?}");

                    if let Err(e) = tx.try_send(key_event) {
                        log::warn!("mouse tap: failed to send event: {e}");
                        return Some(event.clone());
                    }

                    // Suppress the original event so it doesn't reach the system.
                    None
                },
            ) {
                Ok(tap) => tap,
                Err(()) => {
                    log::error!(
                        "Failed to create mouse event tap. \
                         Ensure kanata has Accessibility or Input Monitoring permission \
                         in System Settings > Privacy & Security."
                    );
                    // Release the install claim so a future live reload can
                    // retry once the user grants permission.
                    MOUSE_TAP_INSTALLED.store(false, Ordering::Release);
                    return;
                }
            };

            let Ok(loop_source) = tap.mach_port.create_runloop_source(0) else {
                log::error!("failed to create CFRunLoop source for mouse event tap");
                MOUSE_TAP_INSTALLED.store(false, Ordering::Release);
                return;
            };
            // Safety: kCFRunLoopCommonModes is an extern static from CoreFoundation.
            // Accessing it requires unsafe but is always valid in a running process.
            let mode = unsafe { kCFRunLoopCommonModes };
            CFRunLoop::get_current().add_source(&loop_source, mode);
            tap.enable();
            // MOUSE_TAP_INSTALLED was already set by the caller via
            // compare_exchange before this thread was spawned.
            log::info!("Mouse event tap installed and active.");
            CFRunLoop::run_current();
        });

    match spawn_result {
        Ok(handle) => Some(handle),
        Err(e) => {
            log::error!("failed to spawn mouse event tap thread: {e}");
            MOUSE_TAP_INSTALLED.store(false, Ordering::Release);
            None
        }
    }
}

/// Re-attempt installing the mouse event tap after a live reload. The running
/// tap callback already reads `MAPPED_KEYS` and `MOUSE_MOVEMENT_KEY` live, so
/// if the tap is already up there is nothing to do — but if a reload introduces
/// the first mouse key in defsrc or the first `mouse-movement-key` value, the
/// startup-time install gate may have skipped installation, and we need to
/// install now.
pub fn ensure_mouse_listener_installed_after_reload() {
    if MOUSE_TAP_INSTALLED.load(Ordering::Acquire) {
        // Existing tap reads both MAPPED_KEYS and MOUSE_MOVEMENT_KEY live.
        return;
    }
    let Some(tx) = MOUSE_TAP_TX.get().cloned() else {
        log::debug!("mouse tap reload hook: no tx stashed yet, skipping");
        return;
    };
    let Some(mmk) = MOUSE_MOVEMENT_KEY.get().cloned() else {
        log::debug!("mouse tap reload hook: no mouse_movement_key stashed yet, skipping");
        return;
    };
    let mapped = crate::kanata::MAPPED_KEYS.lock();
    let _ = start_mouse_listener(tx, &mapped, mmk);
}
