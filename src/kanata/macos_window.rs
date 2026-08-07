use accessibility_sys::{
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementCreateSystemWide,
    AXUIElementGetPid, AXUIElementRef, AXUIElementSetAttributeValue,
    AXUIElementSetMessagingTimeout, AXValueCreate, AXValueGetValue, error_string, kAXErrorSuccess,
    kAXFocusedApplicationAttribute, kAXFocusedWindowAttribute, kAXMainWindowAttribute,
    kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize,
};
use anyhow::{Result, anyhow, bail};
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::{CGDisplay, CGPoint, CGRect, CGSize};
use core_graphics::window::{
    CGWindowListCopyWindowInfo, kCGNullWindowID, kCGWindowLayer,
    kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerPID,
};
use kanata_parser::custom_action::{MacosWindowFrame, MacosWindowLayout, MacosWindowPreset};
use objc::rc::autoreleasepool;
use objc::runtime::Object;
use objc::{Encode, Encoding, class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

const BASIS_POINTS: f64 = 10_000.0;
const CYCLE_RESET_AFTER: Duration = Duration::from_millis(900);
const WINDOW_REQUEST_MAX_AGE: Duration = Duration::from_secs(2);
const AX_MESSAGING_TIMEOUT: f32 = 1.0;
const WINDOW_REQUEST_QUEUE_CAPACITY: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

unsafe impl Encode for NSRect {
    fn encode() -> Encoding {
        // NSRect is a CGRect typedef on 64-bit macOS. `objc` needs a local type because Rust's
        // orphan rules prevent implementing its Encode trait for core_graphics::CGRect.
        unsafe { Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

#[derive(Clone, Copy, Debug)]
struct Screen {
    full_frame: CGRect,
    work_area: CGRect,
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[derive(Debug, Default)]
struct CycleState {
    layouts: Vec<MacosWindowLayout>,
    window: Option<CFType>,
    step: usize,
    achieved_frame: Option<CGRect>,
    last_used: Option<Instant>,
}

impl CycleState {
    fn next_index(
        &self,
        layouts: &[MacosWindowLayout],
        window: &CFType,
        current_frame: CGRect,
        now: Instant,
    ) -> usize {
        let should_continue = self.layouts == layouts
            && self.window.as_ref() == Some(window)
            && self
                .last_used
                .and_then(|last_used| now.checked_duration_since(last_used))
                .is_some_and(|elapsed| elapsed <= CYCLE_RESET_AFTER)
            && self
                .achieved_frame
                .is_some_and(|achieved| rects_nearly_equal(achieved, current_frame));

        if should_continue {
            (self.step + 1) % layouts.len()
        } else {
            0
        }
    }

    fn record(
        &mut self,
        layouts: &[MacosWindowLayout],
        window: &CFType,
        step: usize,
        achieved_frame: CGRect,
        used_at: Instant,
    ) {
        self.layouts.clear();
        self.layouts.extend_from_slice(layouts);
        self.window = Some(window.clone());
        self.step = step;
        self.achieved_frame = Some(achieved_frame);
        self.last_used = Some(used_at);
    }

    fn reset(&mut self) {
        self.layouts.clear();
        self.window = None;
        self.step = 0;
        self.achieved_frame = None;
        self.last_used = None;
    }
}

struct WindowRequest {
    pid: i32,
    layouts: Box<[MacosWindowLayout]>,
    requested_at: Instant,
}

static WINDOW_WORKER: OnceLock<std::result::Result<SyncSender<WindowRequest>, String>> =
    OnceLock::new();

pub fn enqueue_window_layouts(layouts: &'static [MacosWindowLayout]) -> Result<()> {
    if layouts.is_empty() {
        bail!("macos-window action has no layouts");
    }

    let sender = WINDOW_WORKER
        .get_or_init(start_window_worker)
        .as_ref()
        .map_err(|err| anyhow!(err.clone()))?;
    let requested_at = Instant::now();
    let request = WindowRequest {
        pid: focused_application_pid()?,
        // Parser allocations are released on live reload, so queued work must own its data.
        layouts: layouts.into(),
        requested_at,
    };
    match sender.try_send(request) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => {
            bail!("macos-window worker queue is full; dropping layout request")
        }
        Err(TrySendError::Disconnected(_)) => bail!("macos-window worker stopped unexpectedly"),
    }
}

fn start_window_worker() -> std::result::Result<SyncSender<WindowRequest>, String> {
    enable_cocoa_multithreading()?;
    let (sender, receiver) = sync_channel::<WindowRequest>(WINDOW_REQUEST_QUEUE_CAPACITY);
    thread::Builder::new()
        .name("kanata-macos-window".into())
        .spawn(move || {
            let mut cycle_state = CycleState::default();
            for request in receiver {
                if request.requested_at.elapsed() > WINDOW_REQUEST_MAX_AGE {
                    cycle_state.reset();
                    log::warn!("macos-window dropped a stale layout request");
                    continue;
                }
                let _timeout = AxMessagingTimeout::install();
                if let Err(err) = apply_window_layouts(request, &mut cycle_state) {
                    cycle_state.reset();
                    log::error!("macos-window action failed: {err}");
                }
            }
        })
        .map_err(|err| format!("failed to start macos-window worker: {err}"))?;
    Ok(sender)
}

fn enable_cocoa_multithreading() -> std::result::Result<(), String> {
    autoreleasepool(|| unsafe {
        // Rust creates POSIX threads directly. Apple requires one NSThread-created thread before
        // Cocoa is used from secondary threads so that Cocoa enables its internal synchronization.
        let center: *mut Object = msg_send![class!(NSNotificationCenter), defaultCenter];
        let observer: *mut Object = msg_send![class!(NSObject), new];
        if center.is_null() || observer.is_null() {
            return Err("failed to initialize Cocoa threading support".into());
        }
        let _: () = msg_send![
            class!(NSThread),
            detachNewThreadSelector: sel!(removeObserver:)
            toTarget: center
            withObject: observer
        ];
        let _: () = msg_send![observer, release];

        let is_multithreaded: bool = msg_send![class!(NSThread), isMultiThreaded];
        if !is_multithreaded {
            return Err("Cocoa did not enter multithreaded mode".into());
        }
        Ok(())
    })
}

struct AxMessagingTimeout {
    system_wide: CFType,
    installed: bool,
}

impl AxMessagingTimeout {
    fn install() -> Option<Self> {
        // SAFETY: the create function returns either null or an owned CF object.
        let system_wide = unsafe { AXUIElementCreateSystemWide() };
        if system_wide.is_null() {
            log::warn!("macos-window could not create the system-wide AX element");
            return None;
        }
        // SAFETY: `system_wide` is non-null and follows the create rule.
        let system_wide = unsafe { CFType::wrap_under_create_rule(system_wide as CFTypeRef) };
        // Apple documents the system-wide element as setting this process's global AX timeout.
        let err = unsafe {
            AXUIElementSetMessagingTimeout(
                system_wide.as_CFTypeRef() as AXUIElementRef,
                AX_MESSAGING_TIMEOUT,
            )
        };
        if err != kAXErrorSuccess {
            log::warn!(
                "macos-window could not set the AX messaging timeout: {} ({err})",
                error_string(err)
            );
        }
        Some(Self {
            system_wide,
            installed: err == kAXErrorSuccess,
        })
    }
}

impl Drop for AxMessagingTimeout {
    fn drop(&mut self) {
        if self.installed {
            // A zero timeout on the system-wide element restores Apple's default.
            let err = unsafe {
                AXUIElementSetMessagingTimeout(
                    self.system_wide.as_CFTypeRef() as AXUIElementRef,
                    0.0,
                )
            };
            if err != kAXErrorSuccess {
                log::warn!(
                    "macos-window could not restore the AX messaging timeout: {} ({err})",
                    error_string(err)
                );
            }
        }
    }
}

fn apply_window_layouts(request: WindowRequest, cycle_state: &mut CycleState) -> Result<()> {
    let window = focused_window(request.pid)?;
    let current_frame = window_frame(window.element)?;
    let screen = screen_for_rect(current_frame)?;
    let step = cycle_state.next_index(
        &request.layouts,
        &window.window,
        current_frame,
        request.requested_at,
    );
    let requested_frame = resolve_layout(screen, current_frame, request.layouts[step]);
    let achieved_frame = set_window_frame(window.element, current_frame, requested_frame)?;
    cycle_state.record(
        &request.layouts,
        &window.window,
        step,
        achieved_frame,
        request.requested_at,
    );
    Ok(())
}

struct FocusedWindow {
    _app: CFType,
    window: CFType,
    element: AXUIElementRef,
}

fn focused_window(pid: i32) -> Result<FocusedWindow> {
    let app = application_for_pid(pid)?;
    let app_element = app.as_CFTypeRef() as AXUIElementRef;
    let window = copy_ax_attr(app_element, kAXFocusedWindowAttribute)
        .or_else(|_| copy_ax_attr(app_element, kAXMainWindowAttribute))?;
    let element = window.as_CFTypeRef() as AXUIElementRef;
    Ok(FocusedWindow {
        _app: app,
        window,
        element,
    })
}

fn application_for_pid(pid: i32) -> Result<CFType> {
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        bail!("AXUIElementCreateApplication({pid}) returned null");
    }
    Ok(unsafe { CFType::wrap_under_create_rule(app as CFTypeRef) })
}

fn focused_application_pid() -> Result<i32> {
    // WindowServer orders visible windows front-to-back and remains current in a system
    // LaunchDaemon. NSWorkspace can lag behind app activation there, while the system-wide AX
    // focused-application attribute is not exposed at all in that execution context.
    match window_server_frontmost_application_pid() {
        Ok(pid) => Ok(pid),
        Err(window_server_err) => match ax_focused_application_pid() {
            Ok(pid) => Ok(pid),
            Err(ax_err) => frontmost_application_pid().map_err(|workspace_err| {
                anyhow!(
                    "could not determine the focused application: WindowServer: \
                     {window_server_err:#}; Accessibility: {ax_err:#}; NSWorkspace: \
                     {workspace_err:#}"
                )
            }),
        },
    }
}

fn window_server_frontmost_application_pid() -> Result<i32> {
    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let windows = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };
    if windows.is_null() {
        bail!("CGWindowListCopyWindowInfo returned null");
    }
    let windows: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { CFArray::wrap_under_create_rule(windows) };

    for window in windows.iter() {
        if window_info_i32(&window, unsafe { kCGWindowLayer }) != Some(0) {
            continue;
        }
        if let Some(pid) =
            window_info_i32(&window, unsafe { kCGWindowOwnerPID }).filter(|pid| *pid > 0)
        {
            return Ok(pid);
        }
    }
    bail!("CGWindowListCopyWindowInfo returned no normal application windows")
}

fn window_info_i32(
    window: &CFDictionary<CFString, CFType>,
    key: core_foundation::string::CFStringRef,
) -> Option<i32> {
    let key = unsafe { CFString::wrap_under_get_rule(key) };
    window.find(&key)?.downcast::<CFNumber>()?.to_i32()
}

fn ax_focused_application_pid() -> Result<i32> {
    let system_wide = unsafe { AXUIElementCreateSystemWide() };
    if system_wide.is_null() {
        bail!("AXUIElementCreateSystemWide returned null");
    }
    let system_wide = unsafe { CFType::wrap_under_create_rule(system_wide as CFTypeRef) };
    let application = copy_ax_attr(
        system_wide.as_CFTypeRef() as AXUIElementRef,
        kAXFocusedApplicationAttribute,
    )?;

    let mut pid = 0;
    let err = unsafe { AXUIElementGetPid(application.as_CFTypeRef() as AXUIElementRef, &mut pid) };
    if err != kAXErrorSuccess {
        bail!(
            "AXUIElementGetPid(AXFocusedApplication) failed: {} ({err})",
            error_string(err)
        );
    }
    if pid <= 0 {
        bail!("AXUIElementGetPid(AXFocusedApplication) returned invalid PID {pid}");
    }
    Ok(pid)
}

fn frontmost_application_pid() -> Result<i32> {
    autoreleasepool(|| unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            bail!("NSWorkspace.sharedWorkspace returned null");
        }
        let application: *mut Object = msg_send![workspace, frontmostApplication];
        if application.is_null() {
            bail!("NSWorkspace.frontmostApplication returned null");
        }
        let pid: i32 = msg_send![application, processIdentifier];
        if pid <= 0 {
            bail!("NSWorkspace.frontmostApplication returned invalid PID {pid}");
        }
        Ok(pid)
    })
}

fn copy_ax_attr(element: AXUIElementRef, attr: &'static str) -> Result<CFType> {
    let attr_ref = CFString::from_static_string(attr);
    let mut value: CFTypeRef = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(element, attr_ref.as_concrete_TypeRef(), &mut value)
    };
    if err != kAXErrorSuccess {
        bail!(
            "AXUIElementCopyAttributeValue({attr}) failed: {} ({err})",
            error_string(err)
        );
    }
    if value.is_null() {
        bail!("AXUIElementCopyAttributeValue({attr}) returned null");
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

fn window_frame(window: AXUIElementRef) -> Result<CGRect> {
    let position = ax_point(window, kAXPositionAttribute)?;
    let size = ax_size(window, kAXSizeAttribute)?;
    Ok(CGRect::new(&position, &size))
}

fn screen_for_rect(rect: CGRect) -> Result<Screen> {
    select_screen(rect, &active_screens()?).ok_or_else(|| anyhow!("macOS reported no displays"))
}

fn select_screen(rect: CGRect, screens: &[Screen]) -> Option<Screen> {
    let (&first, rest) = screens.split_first()?;
    let mut best = first;
    let mut best_overlap = intersection_area(rect, first.full_frame);

    for &screen in rest {
        let overlap = intersection_area(rect, screen.full_frame);
        if overlap > best_overlap {
            best = screen;
            best_overlap = overlap;
        }
    }
    if best_overlap > 0.0 {
        return Some(best);
    }

    let center = rect_center(rect);
    let mut best_distance = squared_distance_to_rect(center, first.full_frame);
    for &screen in rest {
        let distance = squared_distance_to_rect(center, screen.full_frame);
        if distance < best_distance {
            best = screen;
            best_distance = distance;
        }
    }
    Some(best)
}

fn intersection_area(left: CGRect, right: CGRect) -> f64 {
    let width =
        (rect_max_x(left).min(rect_max_x(right)) - left.origin.x.max(right.origin.x)).max(0.0);
    let height =
        (rect_max_y(left).min(rect_max_y(right)) - left.origin.y.max(right.origin.y)).max(0.0);
    width * height
}

fn rect_center(rect: CGRect) -> CGPoint {
    CGPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

fn squared_distance_to_rect(point: CGPoint, rect: CGRect) -> f64 {
    let dx = if point.x < rect.origin.x {
        rect.origin.x - point.x
    } else if point.x > rect_max_x(rect) {
        point.x - rect_max_x(rect)
    } else {
        0.0
    };
    let dy = if point.y < rect.origin.y {
        rect.origin.y - point.y
    } else if point.y > rect_max_y(rect) {
        point.y - rect_max_y(rect)
    } else {
        0.0
    };
    dx * dx + dy * dy
}

fn rect_max_x(rect: CGRect) -> f64 {
    rect.origin.x + rect.size.width
}

fn rect_max_y(rect: CGRect) -> f64 {
    rect.origin.y + rect.size.height
}

fn active_screens() -> Result<Vec<Screen>> {
    // NSScreen.visibleFrame is the public source for the usable area after the menu bar and Dock.
    // Core Graphics only provides the full display bounds, so use that solely as a fallback.
    let screens = appkit_visible_screens();
    if !screens.is_empty() {
        return Ok(screens);
    }

    Ok(active_display_bounds()?
        .into_iter()
        .map(|display| Screen {
            full_frame: display,
            work_area: display,
        })
        .collect())
}

fn appkit_visible_screens() -> Vec<Screen> {
    autoreleasepool(|| unsafe {
        let screens: *mut Object = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return Vec::new();
        }

        let count: usize = msg_send![screens, count];
        if count == 0 {
            return Vec::new();
        }
        let zero_screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        if zero_screen.is_null() {
            return Vec::new();
        }
        let zero_frame: NSRect = msg_send![zero_screen, frame];
        let zero_screen_top = zero_frame.origin.y + zero_frame.size.height;
        let mut result = Vec::with_capacity(count);

        for index in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: index];
            if screen.is_null() {
                continue;
            }

            let ns_frame: NSRect = msg_send![screen, frame];
            let visible_frame: NSRect = msg_send![screen, visibleFrame];
            result.push(Screen {
                full_frame: appkit_rect_to_ax_rect(ns_frame, zero_screen_top),
                work_area: appkit_rect_to_ax_rect(visible_frame, zero_screen_top),
            });
        }

        result
    })
}

fn appkit_rect_to_ax_rect(rect: NSRect, zero_screen_top: f64) -> CGRect {
    // AppKit's global coordinates are y-up; Accessibility window coordinates are y-down from
    // the top of the zero screen. X coordinates and logical point sizes are already compatible.
    CGRect::new(
        &CGPoint::new(
            rect.origin.x,
            zero_screen_top - rect.origin.y - rect.size.height,
        ),
        &CGSize::new(rect.size.width, rect.size.height),
    )
}

fn active_display_bounds() -> Result<Vec<CGRect>> {
    let displays = CGDisplay::active_displays()
        .map_err(|err| anyhow!("CGGetActiveDisplayList failed with CGError {err}"))?;
    Ok(displays
        .into_iter()
        .map(|display| CGDisplay::new(display).bounds())
        .collect())
}

fn ax_point(element: AXUIElementRef, attr: &'static str) -> Result<CGPoint> {
    let value = copy_ax_attr(element, attr)?;
    let mut point = CGPoint::new(0.0, 0.0);
    let ok = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef() as _,
            kAXValueTypeCGPoint,
            &mut point as *mut CGPoint as *mut c_void,
        )
    };
    ok.then_some(point)
        .ok_or_else(|| anyhow!("AXValueGetValue({attr}) did not contain a CGPoint"))
}

fn ax_size(element: AXUIElementRef, attr: &'static str) -> Result<CGSize> {
    let value = copy_ax_attr(element, attr)?;
    let mut size = CGSize::new(0.0, 0.0);
    let ok = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef() as _,
            kAXValueTypeCGSize,
            &mut size as *mut CGSize as *mut c_void,
        )
    };
    ok.then_some(size)
        .ok_or_else(|| anyhow!("AXValueGetValue({attr}) did not contain a CGSize"))
}

fn resolve_layout(screen: Screen, current_frame: CGRect, layout: MacosWindowLayout) -> CGRect {
    match layout {
        MacosWindowLayout::Frame(frame) => resolve_frame(screen.work_area, frame),
        MacosWindowLayout::Preset(preset) => resolve_preset(screen, current_frame, preset),
    }
}

fn resolve_preset(screen: Screen, current_frame: CGRect, preset: MacosWindowPreset) -> CGRect {
    // WindowManager.framework has private native-tiling requests, but those create macOS tile
    // groups and apply the user's tiling margins. This action promises exact visible-area frames
    // (including arbitrary basis-point frames), so its geometry deliberately stays independent.
    use MacosWindowPreset::*;
    let work_area = screen.work_area;
    match preset {
        Maximize => grid_rect(work_area, 0, 0, 1, 1),
        AlmostMaximize => resolve_frame(
            work_area,
            MacosWindowFrame {
                x: 500,
                y: 500,
                width: 9_000,
                height: 9_000,
            },
        ),
        LeftHalf => grid_rect(work_area, 0, 0, 2, 1),
        RightHalf => grid_rect(work_area, 1, 0, 2, 1),
        TopHalf => grid_rect(work_area, 0, 0, 1, 2),
        BottomHalf => grid_rect(work_area, 0, 1, 1, 2),
        Center => center_current_window(work_area, current_frame),
        FirstThird => grid_rect(work_area, 0, 0, 3, 1),
        CenterThird => grid_rect(work_area, 1, 0, 3, 1),
        LastThird => grid_rect(work_area, 2, 0, 3, 1),
        LeftTwoThirds => span_rect(work_area, 0, 0, 2, 1, 3, 1),
        CenterTwoThirds => span_rect(work_area, 0.5, 0.0, 2.0, 1.0, 3.0, 1.0),
        RightTwoThirds => span_rect(work_area, 1, 0, 2, 1, 3, 1),
        FirstThreeFourths => span_rect(work_area, 0, 0, 3, 1, 4, 1),
        CenterThreeFourths => span_rect(work_area, 0.5, 0.0, 3.0, 1.0, 4.0, 1.0),
        LastThreeFourths => span_rect(work_area, 1, 0, 3, 1, 4, 1),
        TopThird => grid_rect(work_area, 0, 0, 1, 3),
        MiddleThird => grid_rect(work_area, 0, 1, 1, 3),
        BottomThird => grid_rect(work_area, 0, 2, 1, 3),
        TopTwoThirds => span_rect(work_area, 0, 0, 1, 2, 1, 3),
        BottomTwoThirds => span_rect(work_area, 0, 1, 1, 2, 1, 3),
        TopCenterTwoThirds => span_rect(work_area, 0.5, 0.0, 2.0, 1.0, 3.0, 2.0),
        BottomCenterTwoThirds => span_rect(work_area, 0.5, 1.0, 2.0, 1.0, 3.0, 2.0),
        TopFirstFourth => span_rect(work_area, 0, 0, 1, 1, 4, 2),
        TopSecondFourth => span_rect(work_area, 1, 0, 1, 1, 4, 2),
        TopThirdFourth => span_rect(work_area, 2, 0, 1, 1, 4, 2),
        TopLastFourth => span_rect(work_area, 3, 0, 1, 1, 4, 2),
        TopThreeFourths => span_rect(work_area, 0, 0, 1, 3, 1, 4),
        BottomThreeFourths => span_rect(work_area, 0, 1, 1, 3, 1, 4),
        FirstFourth => grid_rect(work_area, 0, 0, 4, 1),
        SecondFourth => grid_rect(work_area, 1, 0, 4, 1),
        ThirdFourth => grid_rect(work_area, 2, 0, 4, 1),
        LastFourth => grid_rect(work_area, 3, 0, 4, 1),
        TopLeftQuarter => grid_rect(work_area, 0, 0, 2, 2),
        TopRightQuarter => grid_rect(work_area, 1, 0, 2, 2),
        BottomLeftQuarter => grid_rect(work_area, 0, 1, 2, 2),
        BottomRightQuarter => grid_rect(work_area, 1, 1, 2, 2),
        TopLeftSixth => grid_rect(work_area, 0, 0, 3, 2),
        TopCenterSixth => grid_rect(work_area, 1, 0, 3, 2),
        TopRightSixth => grid_rect(work_area, 2, 0, 3, 2),
        BottomLeftSixth => grid_rect(work_area, 0, 1, 3, 2),
        BottomCenterSixth => grid_rect(work_area, 1, 1, 3, 2),
        BottomRightSixth => grid_rect(work_area, 2, 1, 3, 2),
        MoveLeft => move_current_window(work_area, current_frame, MoveEdge::Left),
        MoveRight => move_current_window(work_area, current_frame, MoveEdge::Right),
        MoveTop => move_current_window(work_area, current_frame, MoveEdge::Top),
        MoveBottom => move_current_window(work_area, current_frame, MoveEdge::Bottom),
    }
}

fn center_current_window(screen: CGRect, current_frame: CGRect) -> CGRect {
    let width = current_frame.size.width.min(screen.size.width);
    let height = current_frame.size.height.min(screen.size.height);
    CGRect::new(
        &CGPoint::new(
            screen.origin.x + (screen.size.width - width) / 2.0,
            screen.origin.y + (screen.size.height - height) / 2.0,
        ),
        &CGSize::new(width, height),
    )
}

enum MoveEdge {
    Left,
    Right,
    Top,
    Bottom,
}

fn move_current_window(screen: CGRect, current_frame: CGRect, edge: MoveEdge) -> CGRect {
    let width = current_frame.size.width.min(screen.size.width);
    let height = current_frame.size.height.min(screen.size.height);
    let x = match edge {
        MoveEdge::Left => screen.origin.x,
        MoveEdge::Right => screen.origin.x + screen.size.width - width,
        MoveEdge::Top | MoveEdge::Bottom => clamp_axis(
            current_frame.origin.x,
            screen.origin.x,
            screen.size.width,
            width,
        ),
    };
    let y = match edge {
        MoveEdge::Top => screen.origin.y,
        MoveEdge::Bottom => screen.origin.y + screen.size.height - height,
        MoveEdge::Left | MoveEdge::Right => clamp_axis(
            current_frame.origin.y,
            screen.origin.y,
            screen.size.height,
            height,
        ),
    };
    CGRect::new(&CGPoint::new(x, y), &CGSize::new(width, height))
}

fn clamp_axis(value: f64, origin: f64, span: f64, size: f64) -> f64 {
    if size >= span {
        return origin;
    }
    value.max(origin).min(origin + span - size)
}

fn resolve_frame(screen: CGRect, frame: MacosWindowFrame) -> CGRect {
    CGRect::new(
        &CGPoint::new(
            screen.origin.x + screen.size.width * f64::from(frame.x) / BASIS_POINTS,
            screen.origin.y + screen.size.height * f64::from(frame.y) / BASIS_POINTS,
        ),
        &CGSize::new(
            screen.size.width * f64::from(frame.width) / BASIS_POINTS,
            screen.size.height * f64::from(frame.height) / BASIS_POINTS,
        ),
    )
}

fn grid_rect(screen: CGRect, column: u32, row: u32, columns: u32, rows: u32) -> CGRect {
    span_rect(
        screen,
        f64::from(column),
        f64::from(row),
        1.0,
        1.0,
        f64::from(columns),
        f64::from(rows),
    )
}

fn span_rect(
    screen: CGRect,
    column: impl Into<f64>,
    row: impl Into<f64>,
    column_span: impl Into<f64>,
    row_span: impl Into<f64>,
    columns: impl Into<f64>,
    rows: impl Into<f64>,
) -> CGRect {
    let column = column.into();
    let row = row.into();
    let column_span = column_span.into();
    let row_span = row_span.into();
    let columns = columns.into();
    let rows = rows.into();
    CGRect::new(
        &CGPoint::new(
            screen.origin.x + screen.size.width * column / columns,
            screen.origin.y + screen.size.height * row / rows,
        ),
        &CGSize::new(
            screen.size.width * column_span / columns,
            screen.size.height * row_span / rows,
        ),
    )
}

fn set_window_frame(
    window: AXUIElementRef,
    current_frame: CGRect,
    requested_frame: CGRect,
) -> Result<CGRect> {
    let should_resize = !sizes_nearly_equal(current_frame.size, requested_frame.size);

    // AX exposes position and size separately. Some apps constrain one based on the other,
    // so repeat the size after moving and retry the complete sequence once if needed.
    if should_resize {
        set_ax_size(window, kAXSizeAttribute, requested_frame.size)?;
    }
    set_ax_point(window, kAXPositionAttribute, requested_frame.origin)?;
    if should_resize {
        set_ax_size(window, kAXSizeAttribute, requested_frame.size)?;
    }

    let mut actual = window_frame(window)?;
    if !rects_nearly_equal(actual, requested_frame) {
        if should_resize {
            set_ax_size(window, kAXSizeAttribute, requested_frame.size)?;
        }
        set_ax_point(window, kAXPositionAttribute, requested_frame.origin)?;
        if should_resize {
            set_ax_size(window, kAXSizeAttribute, requested_frame.size)?;
        }
        actual = window_frame(window)?;
    }
    if !rects_nearly_equal(actual, requested_frame) {
        log::debug!("macos-window requested frame {requested_frame:?}, app applied {actual:?}");
    }
    Ok(actual)
}

fn rects_nearly_equal(left: CGRect, right: CGRect) -> bool {
    (left.origin.x - right.origin.x).abs() <= 1.0
        && (left.origin.y - right.origin.y).abs() <= 1.0
        && sizes_nearly_equal(left.size, right.size)
}

fn sizes_nearly_equal(left: CGSize, right: CGSize) -> bool {
    (left.width - right.width).abs() <= 1.0 && (left.height - right.height).abs() <= 1.0
}

fn set_ax_point(element: AXUIElementRef, attr: &'static str, point: CGPoint) -> Result<()> {
    let value = unsafe {
        AXValueCreate(
            kAXValueTypeCGPoint,
            &point as *const CGPoint as *const c_void,
        )
    };
    if value.is_null() {
        bail!("AXValueCreate({attr}) returned null");
    }
    let value = unsafe { CFType::wrap_under_create_rule(value as CFTypeRef) };
    set_ax_attr(element, attr, value.as_CFTypeRef())
}

fn set_ax_size(element: AXUIElementRef, attr: &'static str, size: CGSize) -> Result<()> {
    let value =
        unsafe { AXValueCreate(kAXValueTypeCGSize, &size as *const CGSize as *const c_void) };
    if value.is_null() {
        bail!("AXValueCreate({attr}) returned null");
    }
    let value = unsafe { CFType::wrap_under_create_rule(value as CFTypeRef) };
    set_ax_attr(element, attr, value.as_CFTypeRef())
}

fn set_ax_attr(element: AXUIElementRef, attr: &'static str, value: CFTypeRef) -> Result<()> {
    let attr_ref = CFString::from_static_string(attr);
    let err =
        unsafe { AXUIElementSetAttributeValue(element, attr_ref.as_concrete_TypeRef(), value) };
    if err != kAXErrorSuccess {
        bail!(
            "AXUIElementSetAttributeValue({attr}) failed: {} ({err})",
            error_string(err)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> CGRect {
        CGRect::new(&CGPoint::new(x, y), &CGSize::new(width, height))
    }

    fn screen(x: f64, y: f64, width: f64, height: f64) -> Screen {
        let frame = rect(x, y, width, height);
        Screen {
            full_frame: frame,
            work_area: frame,
        }
    }

    #[test]
    fn cocoa_multithreading_is_enabled_for_worker() {
        enable_cocoa_multithreading().unwrap();
        let is_multithreaded: bool = unsafe { msg_send![class!(NSThread), isMultiThreaded] };
        assert!(is_multithreaded);
    }

    #[test]
    fn screen_selection_uses_largest_overlap() {
        let screens = [
            screen(0.0, 0.0, 1_000.0, 800.0),
            screen(1_000.0, 0.0, 800.0, 800.0),
        ];
        let selected = select_screen(rect(900.0, 100.0, 400.0, 400.0), &screens).unwrap();
        assert_eq!(selected.full_frame.origin.x, 1_000.0);
    }

    #[test]
    fn screen_selection_uses_nearest_screen_when_window_is_offscreen() {
        let screens = [
            screen(0.0, 0.0, 1_000.0, 800.0),
            screen(1_000.0, 0.0, 800.0, 800.0),
        ];
        let selected = select_screen(rect(1_900.0, 100.0, 100.0, 100.0), &screens).unwrap();
        assert_eq!(selected.full_frame.origin.x, 1_000.0);
    }

    #[test]
    fn appkit_screen_coordinates_convert_to_ax_coordinates() {
        let above = NSRect {
            origin: NSPoint { x: 0.0, y: 900.0 },
            size: NSSize {
                width: 1_920.0,
                height: 1_080.0,
            },
        };
        let below = NSRect {
            origin: NSPoint {
                x: 0.0,
                y: -1_080.0,
            },
            size: NSSize {
                width: 1_920.0,
                height: 1_080.0,
            },
        };

        assert_eq!(appkit_rect_to_ax_rect(above, 900.0).origin.y, -1_080.0);
        assert_eq!(appkit_rect_to_ax_rect(below, 900.0).origin.y, 900.0);
    }

    #[test]
    fn layout_cycle_resets_after_window_moves_focus_changes_or_timeout() {
        let layouts = [
            MacosWindowLayout::Preset(MacosWindowPreset::LeftHalf),
            MacosWindowLayout::Preset(MacosWindowPreset::RightHalf),
            MacosWindowLayout::Preset(MacosWindowPreset::Maximize),
        ];
        let window = CFString::new("window").into_CFType();
        let other_window = CFString::new("other-window").into_CFType();
        let start = Instant::now();
        let achieved = rect(0.0, 0.0, 500.0, 800.0);
        let mut state = CycleState::default();

        assert_eq!(state.next_index(&layouts, &window, achieved, start), 0);
        state.record(&layouts, &window, 0, achieved, start);
        assert_eq!(
            state.next_index(
                &layouts,
                &window,
                achieved,
                start + Duration::from_millis(100)
            ),
            1
        );
        assert_eq!(
            state.next_index(
                &layouts,
                &other_window,
                achieved,
                start + Duration::from_millis(100)
            ),
            0
        );
        assert_eq!(
            state.next_index(
                &layouts,
                &window,
                rect(10.0, 0.0, 500.0, 800.0),
                start + Duration::from_millis(100)
            ),
            0
        );
        assert_eq!(
            state.next_index(
                &layouts,
                &window,
                achieved,
                start + CYCLE_RESET_AFTER + Duration::from_millis(1)
            ),
            0
        );
    }

    #[test]
    fn center_and_edge_moves_keep_window_inside_work_area() {
        let work_area = rect(100.0, 50.0, 1_000.0, 700.0);
        let oversized = rect(-50.0, -50.0, 1_200.0, 900.0);

        assert!(rects_nearly_equal(
            center_current_window(work_area, oversized),
            work_area
        ));
        assert!(rects_nearly_equal(
            move_current_window(work_area, oversized, MoveEdge::Right),
            work_area
        ));
    }

    #[test]
    fn every_preset_resolves_to_its_named_region() {
        use MacosWindowPreset::*;

        let display = screen(100.0, 50.0, 1_200.0, 800.0);
        let current = rect(250.0, 200.0, 300.0, 200.0);
        let third_height = 800.0 / 3.0;
        let cases = [
            (Maximize, rect(100.0, 50.0, 1_200.0, 800.0)),
            (AlmostMaximize, rect(160.0, 90.0, 1_080.0, 720.0)),
            (LeftHalf, rect(100.0, 50.0, 600.0, 800.0)),
            (RightHalf, rect(700.0, 50.0, 600.0, 800.0)),
            (TopHalf, rect(100.0, 50.0, 1_200.0, 400.0)),
            (BottomHalf, rect(100.0, 450.0, 1_200.0, 400.0)),
            (Center, rect(550.0, 350.0, 300.0, 200.0)),
            (FirstThird, rect(100.0, 50.0, 400.0, 800.0)),
            (CenterThird, rect(500.0, 50.0, 400.0, 800.0)),
            (LastThird, rect(900.0, 50.0, 400.0, 800.0)),
            (LeftTwoThirds, rect(100.0, 50.0, 800.0, 800.0)),
            (CenterTwoThirds, rect(300.0, 50.0, 800.0, 800.0)),
            (RightTwoThirds, rect(500.0, 50.0, 800.0, 800.0)),
            (FirstThreeFourths, rect(100.0, 50.0, 900.0, 800.0)),
            (CenterThreeFourths, rect(250.0, 50.0, 900.0, 800.0)),
            (LastThreeFourths, rect(400.0, 50.0, 900.0, 800.0)),
            (TopThird, rect(100.0, 50.0, 1_200.0, third_height)),
            (
                MiddleThird,
                rect(100.0, 50.0 + third_height, 1_200.0, third_height),
            ),
            (
                BottomThird,
                rect(100.0, 50.0 + 2.0 * third_height, 1_200.0, third_height),
            ),
            (TopTwoThirds, rect(100.0, 50.0, 1_200.0, 2.0 * third_height)),
            (
                BottomTwoThirds,
                rect(100.0, 50.0 + third_height, 1_200.0, 2.0 * third_height),
            ),
            (TopCenterTwoThirds, rect(300.0, 50.0, 800.0, 400.0)),
            (BottomCenterTwoThirds, rect(300.0, 450.0, 800.0, 400.0)),
            (TopFirstFourth, rect(100.0, 50.0, 300.0, 400.0)),
            (TopSecondFourth, rect(400.0, 50.0, 300.0, 400.0)),
            (TopThirdFourth, rect(700.0, 50.0, 300.0, 400.0)),
            (TopLastFourth, rect(1_000.0, 50.0, 300.0, 400.0)),
            (TopThreeFourths, rect(100.0, 50.0, 1_200.0, 600.0)),
            (BottomThreeFourths, rect(100.0, 250.0, 1_200.0, 600.0)),
            (FirstFourth, rect(100.0, 50.0, 300.0, 800.0)),
            (SecondFourth, rect(400.0, 50.0, 300.0, 800.0)),
            (ThirdFourth, rect(700.0, 50.0, 300.0, 800.0)),
            (LastFourth, rect(1_000.0, 50.0, 300.0, 800.0)),
            (TopLeftQuarter, rect(100.0, 50.0, 600.0, 400.0)),
            (TopRightQuarter, rect(700.0, 50.0, 600.0, 400.0)),
            (BottomLeftQuarter, rect(100.0, 450.0, 600.0, 400.0)),
            (BottomRightQuarter, rect(700.0, 450.0, 600.0, 400.0)),
            (TopLeftSixth, rect(100.0, 50.0, 400.0, 400.0)),
            (TopCenterSixth, rect(500.0, 50.0, 400.0, 400.0)),
            (TopRightSixth, rect(900.0, 50.0, 400.0, 400.0)),
            (BottomLeftSixth, rect(100.0, 450.0, 400.0, 400.0)),
            (BottomCenterSixth, rect(500.0, 450.0, 400.0, 400.0)),
            (BottomRightSixth, rect(900.0, 450.0, 400.0, 400.0)),
            (MoveLeft, rect(100.0, 200.0, 300.0, 200.0)),
            (MoveRight, rect(1_000.0, 200.0, 300.0, 200.0)),
            (MoveTop, rect(250.0, 50.0, 300.0, 200.0)),
            (MoveBottom, rect(250.0, 650.0, 300.0, 200.0)),
        ];

        for (preset, expected) in cases {
            let actual = resolve_preset(display, current, preset);
            assert!(
                rects_nearly_equal(actual, expected),
                "{preset:?}: expected {expected:?}, got {actual:?}"
            );
        }
    }

    #[test]
    fn custom_frames_are_basis_points_relative_to_the_work_area() {
        let work_area = rect(100.0, 50.0, 1_000.0, 700.0);
        let actual = resolve_frame(
            work_area,
            MacosWindowFrame {
                x: -1_000,
                y: 2_500,
                width: 5_000,
                height: 12_000,
            },
        );

        assert!(rects_nearly_equal(actual, rect(0.0, 225.0, 500.0, 840.0)));
    }
}
