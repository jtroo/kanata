/// Mouse functionality for windows hooks.
///
/// Code taken and adapted from:
/// https://github.com/myood/willhook-rs/tree/f4ccbc897504834d0c01fa449a2660b9989290e5
/// License: MIT
use super::*;

use winapi::shared::windef::HHOOK;
use winapi::um::winuser::UnhookWindowsHookEx;

type MHookFn = dyn FnMut(MouseEventType) -> bool;
thread_local! {
    /// Stores the hook callback for the current thread.
    static MHOOK: Cell<Option<Box<MHookFn>>> = Cell::default();
}

/// Wrapper for the low-level keyboard hook API.
/// Automatically unregisters the hook when dropped.
pub struct MouseHook {
    handle: HHOOK,
}

impl MouseHook {
    pub fn set_input_cb(callback: impl FnMut(MouseEventType) -> bool + 'static) -> MouseHook {
        MHOOK.with(|state| {
            assert!(
                state.take().is_none(),
                "Only one mouse hook can be registered per thread."
            );

            state.set(Some(Box::new(callback)));

            MouseHook {
                handle: unsafe {
                    SetWindowsHookExW(WH_MOUSE_LL, Some(mhook_proc), ptr::null_mut(), 0)
                        .as_mut()
                        .expect("install low-level keyboard hook successfully")
                },
            }
        })
    }
}

impl Drop for MouseHook {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.handle) };
        MHOOK.with(|state| state.take());
    }
}

unsafe extern "system" fn mhook_proc(code: c_int, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mouse_lparam = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
    let is_injected = mouse_lparam.flags & (LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED) != 0;
    log::trace!("{code} {wparam} {is_injected}");

    // Regarding is_injected check:
    // `SendInput()` internally calls the hook function.
    // Filter out injected events to prevent infinite recursion.
    if is_injected {
        return unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) };
    }

    let mut handled = false;
    let mouse_event = unsafe { classify_mouse_event(wparam, mouse_lparam) };
    MHOOK.with(|state| {
        // The unwrap cannot fail, because we have initialized [`HOOK`] with a
        // valid closure before registering the hook (this function).
        // To access the closure we move it out of the cell and put it back
        // after it returned. For this to work we need to prevent recursion by
        // dropping injected events. Otherwise we would try to take the closure
        // twice and the call would fail the second time.
        let mut hook = state.take().expect("no recurse");
        handled = hook(mouse_event);
        state.set(Some(hook));
    });

    if handled {
        1
    } else {
        unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
    }
}

/// The type of the mouse event with it's specific data
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub enum MouseEventType {
    /// Button on the mouse was pressed
    Press(MousePressEvent),
    /// Mouse was moved
    Move(MouseMoveEvent),
    /// Wheel on the mouse was, well, spinning.
    Wheel(MouseWheelEvent),
    /// Received unrecognized mouse event type, the code is stored for reference.
    Other(usize),
}

use MouseEventType::*;

/// Holds information which button was pressed or released
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub struct MousePressEvent {
    pub pressed: MouseButtonPress,
    pub button: MouseButton,
}

/// Holds information which mouse wheel triggered the event
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub enum MouseWheel {
    Horizontal,
    Vertical,
    Unknown(usize),
}

/// Indicates the direction of the mouse wheel spin
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub enum MouseWheelDirection {
    Forward,
    Backward,
    Unknown(u32),
}

/// The mouse wheel event with information which wheel triggered an event and the direction of the spin
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub struct MouseWheelEvent {
    pub wheel: MouseWheel,
    pub direction: Option<MouseWheelDirection>,
}

/// Point in per-monitor aware coordinates, see
/// [MSDN](https://learn.microsoft.com/en-us/windows/desktop/api/shellscalingapi/ne-shellscalingapi-process_dpi_awareness)
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Holds the new cursor position after mouse move
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub struct MouseMoveEvent {
    pub point: Option<Point>,
}

/// Indicates if button was pressed or released
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub enum MouseButtonPress {
    Down,
    Up,
    Other(usize),
}

/// Indicates if mouse button press is single or double click
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub enum MouseClick {
    SingleClick,
    DoubleClick,
    Other(u32),
}

/// Identifies which mouse button triggered an event
#[derive(Copy, Clone, Ord, PartialOrd, Hash, Eq, PartialEq, Debug)]
pub enum MouseButton {
    Left(MouseClick),
    Right(MouseClick),
    Middle(MouseClick),
    /// XBUTTON1
    X1(MouseClick),
    /// XBUTTON2
    X2(MouseClick),
    /// Either XBUTTON1 or XBUTTON2
    UnkownX(MouseClick),
    /// Unexpected mouse button. Raw code stored for reference, see MSDN documentation about low-level hooks.
    Other(usize),
}

/// # Safety
/// Pointers must be valid from winapi.
pub unsafe fn classify_mouse_event(
    wm_mouse_param: WPARAM,
    ms_ll_hook_struct: *const MSLLHOOKSTRUCT,
) -> MouseEventType {
    match wm_mouse_param as u32 {
        // Mouse press
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
            Press(unsafe { MousePressEvent::new(wm_mouse_param, ms_ll_hook_struct) })
        }
        WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK => {
            Press(unsafe { MousePressEvent::new(wm_mouse_param, ms_ll_hook_struct) })
        }
        WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MBUTTONDBLCLK => {
            Press(unsafe { MousePressEvent::new(wm_mouse_param, ms_ll_hook_struct) })
        }
        WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK => {
            Press(unsafe { MousePressEvent::new(wm_mouse_param, ms_ll_hook_struct) })
        }

        // Mouse move
        WM_MOUSEMOVE => Move(unsafe { MouseMoveEvent::new(ms_ll_hook_struct) }),

        // Wheel move
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            Wheel(unsafe { MouseWheelEvent::new(wm_mouse_param, ms_ll_hook_struct) })
        }

        _ => Other(wm_mouse_param),
    }
}

impl MousePressEvent {
    /// # Safety
    /// Pointers must be valid from winapi.
    pub unsafe fn new(
        wm_mouse_param: WPARAM,
        ms_ll_hook_struct: *const MSLLHOOKSTRUCT,
    ) -> MousePressEvent {
        MousePressEvent {
            pressed: MouseButtonPress::from(wm_mouse_param),
            button: unsafe { MouseButton::from(wm_mouse_param, ms_ll_hook_struct) },
        }
    }
}

impl From<POINT> for Point {
    fn from(value: POINT) -> Self {
        Point {
            x: value.x,
            y: value.y,
        }
    }
}

impl MouseWheel {
    pub fn new(wm_mouse_param: WPARAM) -> MouseWheel {
        use MouseWheel::*;
        match wm_mouse_param.try_into() {
            Ok(param_u32) => match param_u32 {
                WM_MOUSEWHEEL => Vertical,
                WM_MOUSEHWHEEL => Horizontal,
                _ => Unknown(wm_mouse_param),
            },
            _ => Unknown(wm_mouse_param),
        }
    }
}

impl MouseWheelEvent {
    /// # Safety
    /// Pointers must be valid from winapi.
    pub unsafe fn new(
        wm_mouse_param: WPARAM,
        ms_ll_hook_struct: *const MSLLHOOKSTRUCT,
    ) -> MouseWheelEvent {
        MouseWheelEvent {
            wheel: MouseWheel::new(wm_mouse_param),
            direction: unsafe { MouseWheelDirection::optionally_from(ms_ll_hook_struct) },
        }
    }
}

impl MouseWheelDirection {
    /// # Safety
    /// Pointers must be valid from winapi.
    pub unsafe fn optionally_from(
        ms_ll_hook_struct: *const MSLLHOOKSTRUCT,
    ) -> Option<MouseWheelDirection> {
        if ms_ll_hook_struct.is_null() {
            None
        } else {
            Some(unsafe { MouseWheelDirection::new(&*ms_ll_hook_struct) })
        }
    }

    fn new(ms_ll_hook_struct: &MSLLHOOKSTRUCT) -> MouseWheelDirection {
        use MouseWheelDirection::*;
        let delta = GET_WHEEL_DELTA_WPARAM(ms_ll_hook_struct.mouseData as WPARAM);
        match delta {
            _ if delta > 0 => Forward,
            _ if delta < 0 => Backward,
            _ => Unknown(ms_ll_hook_struct.mouseData),
        }
    }
}

impl MouseMoveEvent {
    /// # Safety
    /// Pointers must be valid from winapi.
    pub unsafe fn new(ms_ll_hook_struct: *const MSLLHOOKSTRUCT) -> MouseMoveEvent {
        if ms_ll_hook_struct.is_null() {
            MouseMoveEvent { point: None }
        } else {
            let msll = unsafe { &*ms_ll_hook_struct };
            let pt = msll.pt;
            MouseMoveEvent {
                point: Some(pt.into()),
            }
        }
    }
}

impl From<WPARAM> for MouseButtonPress {
    fn from(value: WPARAM) -> Self {
        use MouseButtonPress::*;
        match value.try_into() {
            Ok(uv) => match uv {
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => Down,
                WM_RBUTTONUP | WM_LBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP => Up,
                _ => Other(value),
            },
            Err(_) => Other(value),
        }
    }
}

impl From<WPARAM> for MouseClick {
    fn from(value: WPARAM) -> Self {
        use MouseClick::*;
        match value.try_into() {
            Ok(uv) => match uv {
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => SingleClick,
                WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP => SingleClick,
                WM_LBUTTONDBLCLK | WM_RBUTTONDBLCLK | WM_MBUTTONDBLCLK | WM_XBUTTONDBLCLK => {
                    DoubleClick
                }
                _ => Other(value as u32),
            },
            Err(_) => Other(value as u32),
        }
    }
}

impl MouseButton {
    /// # Safety
    /// Pointers must be valid from winapi.
    pub unsafe fn from(wm_mouse_param: WPARAM, ms_ll_hook_struct: *const MSLLHOOKSTRUCT) -> Self {
        let click = MouseClick::from(wm_mouse_param);

        use MouseButton::*;
        match wm_mouse_param.try_into() {
            Ok(param) => {
                match param {
                    WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK => Left(click),
                    WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK => Right(click),
                    WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MBUTTONDBLCLK => Middle(click),
                    WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK => {
                        if ms_ll_hook_struct.is_null() {
                            UnkownX(click)
                        } else {
                            Self::into_extra(click, unsafe { &*ms_ll_hook_struct })
                        }
                    }
                    // Value out of expected set
                    _ => Other(wm_mouse_param),
                }
            }
            // Conversion error
            Err(_) => Other(wm_mouse_param),
        }
    }

    fn into_extra(click: MouseClick, ms_ll_hook_struct: &MSLLHOOKSTRUCT) -> Self {
        use MouseButton::*;
        match GET_XBUTTON_WPARAM(ms_ll_hook_struct.mouseData.try_into().expect("")) {
            XBUTTON1 => X1(click),
            XBUTTON2 => X2(click),
            _ => UnkownX(click),
        }
    }
}

impl TryFrom<MouseEventType> for KeyEvent {
    type Error = ();
    fn try_from(mevt: MouseEventType) -> Result<Self, ()> {
        use OsCode::*;
        match mevt {
            Move(..) | Other(..) => Err(()),
            Press(MousePressEvent { pressed, button }) => {
                let value = match pressed {
                    MouseButtonPress::Up => KeyValue::Release,
                    MouseButtonPress::Down => KeyValue::Press,
                    MouseButtonPress::Other(..) => return Err(()),
                };
                let code = match button {
                    // TODO:
                    // The inner type is MouseClick.
                    // - DoubleClick might need to actually send two events.. but Kanata doesn't
                    //   have a good way to signal multiple events into the event queue for a
                    //   single oskbd source event.
                    // - Other might be better off as Err(())
                    // This applies to all variants.
                    MouseButton::Left(..) => BTN_LEFT,
                    MouseButton::Right(..) => BTN_RIGHT,
                    MouseButton::Middle(..) => BTN_MIDDLE,
                    MouseButton::X1(..) => BTN_SIDE,
                    MouseButton::X2(..) => BTN_EXTRA,
                    MouseButton::UnkownX(..) | MouseButton::Other(..) => return Err(()),
                };
                Ok(KeyEvent { code, value })
            }
            Wheel(MouseWheelEvent { wheel, direction }) => {
                use MouseWheel::*;
                use MouseWheelDirection::*;
                let Some(direction) = direction else {
                    return Err(());
                };
                let code = match (wheel, direction) {
                    (Vertical, Forward) => MouseWheelUp,
                    (Vertical, Backward) => MouseWheelDown,
                    (Horizontal, Forward) => MouseWheelRight,
                    (Horizontal, Backward) => MouseWheelLeft,
                    (MouseWheel::Unknown(..), _) | (_, MouseWheelDirection::Unknown(..)) => {
                        return Err(());
                    }
                };
                Ok(KeyEvent {
                    code,
                    value: KeyValue::Tap,
                })
            }
        }
    }
}
