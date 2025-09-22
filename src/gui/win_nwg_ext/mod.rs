// based on https://github.com/lynxnb/wsl-usb-manager/blob/master/src/gui/nwg_ext.rs
use native_windows_gui as nwg;
use native_windows_gui::ControlHandle;
use std::{mem::size_of, ptr};
use winapi::ctypes::c_int;
use winapi::shared::windef::HWND;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Graphics::Gdi::DeleteObject;
use windows_sys::Win32::UI::Shell::{
    SHGSI_ICON, SHGSI_SMALLICON, SHGetStockIconInfo, SHSTOCKICONID, SHSTOCKICONINFO,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CopyImage, DestroyIcon, GetIconInfoExW, HMENU, ICONINFOEXW, IMAGE_BITMAP, LR_CREATEDIBSECTION,
    MENUITEMINFOW, MF_BYCOMMAND, MIIM_BITMAP, SetMenuItemInfoW,
};

/// Extends [`nwg::Bitmap`] with additional functionality.
pub trait BitmapEx {
    fn from_system_icon(icon: SHSTOCKICONID) -> nwg::Bitmap;
}

impl BitmapEx for nwg::Bitmap {
    /// Creates a bitmap from a [`SHSTOCKICONID`] system icon ID.
    fn from_system_icon(icon: SHSTOCKICONID) -> nwg::Bitmap {
        // Retrieve the icon
        let mut stock_icon_info = SHSTOCKICONINFO {
            cbSize: std::mem::size_of::<SHSTOCKICONINFO>() as u32,
            hIcon: 0,
            iSysImageIndex: 0,
            iIcon: 0,
            szPath: [0; 260],
        };
        unsafe {
            SHGetStockIconInfo(
                icon,
                SHGSI_ICON | SHGSI_SMALLICON,
                &mut stock_icon_info as *mut _,
            );
        }

        // Retrieve the bitmap for the icon
        let mut icon_info = ICONINFOEXW {
            cbSize: std::mem::size_of::<ICONINFOEXW>() as u32,
            fIcon: 0,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: 0,
            hbmColor: 0,
            wResID: 0,
            szModName: [0; 260],
            szResName: [0; 260],
        };
        unsafe {
            GetIconInfoExW(stock_icon_info.hIcon, &mut icon_info as *mut _);
        }

        // Create a copy of the bitmap with transparent background from the icon bitmap
        let hbitmap = unsafe {
            CopyImage(
                icon_info.hbmColor as HANDLE,
                IMAGE_BITMAP,
                0,
                0,
                LR_CREATEDIBSECTION,
            )
        };

        // Delete the unused icon and bitmaps
        unsafe {
            DeleteObject(icon_info.hbmMask);
            DeleteObject(icon_info.hbmColor);
            DestroyIcon(stock_icon_info.hIcon);
        };

        if hbitmap == 0 {
            panic!("Failed to create bitmap from system icon");
        } else {
            #[allow(unused)]
            struct Bitmap {
                handle: HANDLE,
                owned: bool,
            }

            let bitmap = Bitmap {
                handle: hbitmap as HANDLE,
                owned: true,
            };

            // Ugly hack to set the private `owned` field inside nwg::Bitmap to true
            #[allow(clippy::missing_transmute_annotations)]
            unsafe {
                std::mem::transmute(bitmap)
            }
        }
    }
}

/// Extends [`nwg::Menu`] with additional functionality.
pub trait MenuEx {
    fn set_bitmap(&self, bitmap: Option<&nwg::Bitmap>);
}
impl MenuEx for nwg::Menu {
    /// Sets a bitmap to be displayed on a menu. Pass `None` to remove the bitmap
    fn set_bitmap(&self, bitmap: Option<&nwg::Bitmap>) {
        let (hmenu_par, hmenu) = self.handle.hmenu().unwrap();
        let hbitmap = match bitmap {
            Some(b) => b.handle as HANDLE,
            None => 0,
        };

        let menu_item_info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_BITMAP,
            hbmpItem: hbitmap,
            fType: 0,
            fState: 0,
            hSubMenu: 0,
            hbmpChecked: 0,
            hbmpUnchecked: 0,
            dwTypeData: ptr::null_mut(),
            wID: 0,
            dwItemData: 0,
            cch: 0,
        };
        unsafe {
            SetMenuItemInfoW(
                hmenu_par as HMENU,
                hmenu as u32,
                MF_BYCOMMAND as i32,
                &menu_item_info as *const _,
            );
        }
    }
}

/// Extends [`nwg::MenuItem`] with additional functionality.
pub trait MenuItemEx {
    fn set_bitmap(&self, bitmap: Option<&nwg::Bitmap>);
}

impl MenuItemEx for nwg::MenuItem {
    /// Sets a bitmap to be displayed on a menu item. Pass `None` to remove the bitmap.
    fn set_bitmap(&self, bitmap: Option<&nwg::Bitmap>) {
        let (hmenu, item_id) = self.handle.hmenu_item().unwrap();
        let hbitmap = match bitmap {
            Some(b) => b.handle as HANDLE,
            None => 0,
        };

        let menu_item_info = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_BITMAP,
            fType: 0,
            fState: 0,
            wID: 0,
            hSubMenu: 0,
            hbmpChecked: 0,
            hbmpUnchecked: 0,
            dwItemData: 0,
            dwTypeData: std::ptr::null_mut(),
            cch: 0,
            hbmpItem: hbitmap,
        };

        unsafe {
            SetMenuItemInfoW(
                hmenu as HMENU,
                item_id,
                MF_BYCOMMAND as i32,
                &menu_item_info as *const _,
            );
        }
    }
}

pub trait WindowEx {
    fn set_position_ex(&self, x: i32, y: i32);
}
pub fn dpi() -> i32 {
    // prevents GDI DC resource leak
    use winapi::um::wingdi::GetDeviceCaps;
    use winapi::um::wingdi::LOGPIXELSX;
    use winapi::um::winuser::{GetDC, ReleaseDC};
    let screen = unsafe { GetDC(std::ptr::null_mut()) };
    let dpi = unsafe { GetDeviceCaps(screen, LOGPIXELSX) };
    let _ = unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
    dpi
}
pub fn logical_to_physical(x: i32, y: i32) -> (i32, i32) {
    use muldiv::MulDiv;
    use winapi::um::winuser::USER_DEFAULT_SCREEN_DPI;
    let dpi = dpi();
    let x = x.mul_div_round(dpi, USER_DEFAULT_SCREEN_DPI).unwrap_or(x);
    let y = y.mul_div_round(dpi, USER_DEFAULT_SCREEN_DPI).unwrap_or(y);
    (x, y)
}
/// # Safety
/// The `handle` param must be a valid pointer to a window handle returned by some winapi call.
/// Failure to do so probably won't be UB because the handle is passed to a WinAPI call
/// which is expected to handle these cases safely, but seems worth noting anyway.
pub unsafe fn set_window_position(handle: HWND, x: i32, y: i32) {
    use winapi::um::winuser::SetWindowPos;
    use winapi::um::winuser::{SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER};
    let (x, y) = logical_to_physical(x, y);
    unsafe {
        SetWindowPos(
            handle,
            ptr::null_mut(),
            x as c_int,
            y as c_int,
            0,
            0,
            SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
    }
}
const NOT_BOUND: &str = "Window is not yet bound to a winapi object";
const BAD_HANDLE: &str = "INTERNAL ERROR: Window handle is not HWND!";
pub fn check_hwnd(handle: &ControlHandle, not_bound: &str, bad_handle: &str) -> HWND {
    use winapi::um::winuser::IsWindow;
    if handle.blank() {
        panic!("{}", not_bound);
    }
    match handle.hwnd() {
        Some(hwnd) => match unsafe { IsWindow(hwnd) } {
            0 => {
                panic!(
                    "The window handle is no longer valid. This usually means the control was freed by the OS"
                );
            }
            _ => hwnd,
        },
        None => {
            panic!("{}", bad_handle);
        }
    }
}

impl WindowEx for nwg::Window {
    /// Set the position of the button in the parent window
    fn set_position_ex(&self, x: i32, y: i32) {
        let handle = check_hwnd(&self.handle, NOT_BOUND, BAD_HANDLE);
        unsafe { set_window_position(handle, x, y) }
    }
}
