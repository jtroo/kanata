// based on https://github.com/lynxnb/wsl-usb-manager/blob/master/src/gui/nwg_ext.rs
use native_windows_gui as nwg;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Graphics::Gdi::DeleteObject;
use windows_sys::Win32::UI::Shell::{
    SHGetStockIconInfo, SHGSI_ICON, SHGSI_SMALLICON, SHSTOCKICONID, SHSTOCKICONINFO,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CopyImage, DestroyIcon, GetIconInfoExW, SetMenuItemInfoW, HMENU, ICONINFOEXW, IMAGE_BITMAP,
    LR_CREATEDIBSECTION, MENUITEMINFOW, MF_BYCOMMAND, MIIM_BITMAP,
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
            unsafe { std::mem::transmute(bitmap) }
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
        use std::{mem::size_of, ptr};
        let (hmenu_par, hmenu) = self.handle.hmenu().unwrap();
        let hbitmap = match bitmap {
            Some(b) => b.handle as HANDLE,
            None => 0,
        };

        let menu_item_info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32, //u32 byte-size of the structure, must be set by the caller to sizeof(MENUITEMINFO)
            fMask: MIIM_BITMAP, //MENU_ITEM_MASK	members to be retrieved or set:
            //    	  MIIM_BITMAP    	  hbmpItem
            //    	  MIIM_STATE     	  fState
            //    	  MIIM_CHECKMARKS	  hbmpChecked and hbmpUnchecked
            //    	  MIIM_DATA      	  dwItemData
            //    	  MIIM_FTYPE     	  fType
            //    	  MIIM_ID        	  wID
            //    	  MIIM_STRING    	  dwTypeData
            //    	  MIIM_SUBMENU   	  hSubMenu
            //    	  MIIM_TYPE      	  fType and dwTypeData, replaced by MIIM_BITMAP, MIIM_FTYPE, and MIIM_STRING
            hbmpItem: hbitmap, //HBITMAP	bitmap to display or one of: (fMask==MIIM_BITMAP)
            //    	HBMMENU_CALLBACK 	Bitmap that is drawn by the window that owns the menu. The application must process the WM_MEASUREITEM and WM_DRAWITEM messages
            //    	HBMMENU_MBAR_MINIMIZE | _D | HBMMENU_MBAR_RESTORE | HBMMENU_MBAR_CLOSE | _D
            //    		↑ Min|dis Min | Restore	| Close|Disabled close	button for the menu bar
            //    		↓ Min|Max|Close|Restore	                      	button for the submenu
            //    	HBMMENU_POPUP_MINIMIZE | HBMMENU_POPUP_MAXIMIZE | HBMMENU_POPUP_CLOSE | HBMMENU_POPUP_RESTORE
            //    	HBMMENU_SYSTEM    	Windows icon or the icon of the window specified in dwItemData
            fType: 0, //MENU_ITEM_TYPE	requires fMask=MIIM_FTYPE, MFT_BITMAP/MFT_SEPARATOR/MFT_STRING can't be combined, set fMask to MIIM_TYPE to use fType
            //    	  MFT_OWNERDRAW   	  Assigns responsibility for drawing the menu item to the window that owns the menu. The window receives a WM_MEASUREITEM message before the menu is displayed for the first time, and a WM_DRAWITEM message whenever the appearance of the menu item must be updated. If this value is specified, the dwTypeData member contains an application-defined value
            //    	  MFT_MENUBARBREAK	  Places the menu item on a new line (for a menu bar) or in a new column (for a drop-down menu, submenu, or shortcut menu). For a drop-down menu, submenu, or shortcut menu, a vertical line separates the new column from the old
            //    	  MFT_MENUBREAK   	  Places the menu item on a new line (for a menu bar) or in a new column (for a drop-down menu, submenu, or shortcut menu). For a drop-down menu, submenu, or shortcut menu, the columns are not separated by a vertical line
            //    	  MFT_RADIOCHECK  	  Displays selected menu items using a radio-button mark instead of a check mark if the hbmpChecked member is NULL
            //    	  MFT_RIGHTJUSTIFY	  Right-justifies the menu item and any subsequent items. This value is valid only if the menu item is in a menu bar
            //    	  MFT_RIGHTORDER  	  Specifies that menus cascade right-to-left (the default is left-to-right). This is used to support right-to-left languages, such as Arabic and Hebrew
            //    	  MFT_SEPARATOR   	  Specifies that the menu item is a separator. A menu item separator appears as a horizontal dividing line. The dwTypeData and cch members are ignored. This value is valid only in a drop-down menu, submenu, or shortcut menu
            //    	  MFT_STRING      	  Displays the menu item using a text string. dwTypeData member is the pointer to a null-terminated string, and the cch member is the length of the string. replaced by MIIM_STRING
            //    	  MFT_BITMAP      	  Displays the menu item using a bitmap     . low-order word of the dwTypeData member is the bitmap handle, and the cch member is ignored. replaced by MIIM_BITMAP and hbmpItem
            fState: 0, //MENU_ITEM_STATE  requires fMask=MIIM_STATE
            //    	MFS_ENABLED | MFS_DISABLED==MFS_GRAYED	≝Enables|disables&grays so it can|can't be selected
            //    	MFS_CHECKED | MFS_UNCHECKED           	Checks|Unchecks     see hbmpChecked for info
            //    	MFS_HILITE  | MFS_UNHILITE            	Highlights|≝No highlight
            //    	MFS_DEFAULT                           	Set as default (only 1), displayed in bold
            //
            hSubMenu: 0, //HMENU  	handle to the drop-down (sub)menu associated with the menu item. NULL if no drop-down. fMask==MIIM_SUBMENU
            hbmpChecked: 0, //HBITMAP	bitmap to display @     selected item. 0→default bitmap (✓ or • if fType=MFT_RADIOCHECK). fMask==MIIM_CHECKMARKS
            hbmpUnchecked: 0, //HBITMAP	bitmap to display @ not selected item. 0→no bitmap. fMask==MIIM_CHECKMARKS
            dwTypeData: ptr::null_mut(), //PWSTR  	item contents, depends on fType, used if fMask has MIIM_TYPE
            wID: 0,                      //u32    	app-defined item value ID. fMask==MIIM_ID,
            dwItemData: 0,               //usize  	app-defined item value. fMask==MIIM_DATA
            cch: 0,                      //u32    	.
        };
        unsafe {
            SetMenuItemInfoW(
                hmenu_par as HMENU,  //hmenu      	handle to the menu that contains the menu item
                hmenu as u32, //item       	id/pos of the menu item to get infor about, meaning depends on the value of fByPosition
                MF_BYCOMMAND as i32, //fByPosition	meaning of uItem
                // FALSE     	           	   it's a menu item id
                //           	           	   it's a menu item position (see learn.microsoft.com/en-us/windows/desktop/menurc/about-menus)
                &menu_item_info as *const _, //lpmii	pointer to a MENUITEMINFO structure that specifies the information to retrieve and receives information about the menu item (cbSize member must be set to sizeof(MENUITEMINFO) before calling this function)
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
