use crate::Kanata;
use anyhow::{Result, bail};
use core::cell::RefCell;
use kanata_parser::cfg::CfgOptionsGui;
use log::Level::*;
use std::cell::Cell;
use winapi::shared::windef::HWND;

use core::ffi::c_void;
use native_windows_gui as nwg;
use parking_lot::Mutex;
use parking_lot::MutexGuard;

use std::collections::HashMap;
use std::env::{current_exe, var_os};
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender as ASender, TryRecvError};
use std::time::Duration;
use winapi::shared::minwindef::{BYTE, DWORD};
use winapi::shared::windef::COLORREF;
use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, POINT, RECT, SIZE};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows_sys::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CalculatePopupWindowPosition, SM_CXCURSOR, SM_CYCURSOR, TPM_WORKAREA,
};

use crate::gui::win_nwg_ext::{BitmapEx, MenuEx, MenuItemEx};
use kanata_parser::cfg;
use nwg::{ControlHandle, NativeUi};
use std::sync::Arc;

trait PathExt {
    fn add_ext(&mut self, ext_o: impl AsRef<std::path::Path>);
}
impl PathExt for PathBuf {
    fn add_ext(&mut self, ext_o: impl AsRef<std::path::Path>) {
        match self.extension() {
            Some(ext) => {
                let mut ext = ext.to_os_string();
                ext.push(".");
                ext.push(ext_o.as_ref());
                self.set_extension(ext)
            }
            None => self.set_extension(ext_o.as_ref()),
        };
    }
}

#[derive(Default, Debug, Clone)]
pub struct SystemTrayData {
    pub tooltip: String,
    pub cfg_p: Vec<PathBuf>,
    pub cfg_icon: Option<String>,
    pub layer0_name: String,
    pub layer0_icon: Option<String>,
    pub gui_opts: CfgOptionsGui,
    pub tt_duration_pre: u16,
    pub tt_size_pre: (u16, u16),
}
#[derive(Default)]
pub struct Icn {
    pub tray: nwg::Bitmap, // uses an image of different size to fit the menu items
    pub tooltip: nwg::Bitmap, // uses an image of different size to fit the tooltip
    pub icon: nwg::Icon,
}
#[derive(Default)]
pub struct SystemTray {
    pub app_data: RefCell<SystemTrayData>,
    /// Store dynamically created tray menu items
    pub tray_item_dyn: RefCell<Vec<nwg::MenuItem>>,
    /// Store dynamically created tray menu items' handlers
    pub handlers_dyn: RefCell<Vec<nwg::EventHandler>>,
    /// Store dynamically created icons to not load them from a file every time
    /// (icon format for tray icon, bitmap for tray MenuItem icons and tooltips)
    pub img_dyn: RefCell<HashMap<PathBuf, Option<Icn>>>,
    /// Store 'img_dyn' hashmap key for the currently active icon ('cfg_path:🗍layer_name' format)
    pub icon_act_key: RefCell<Option<PathBuf>>,
    /// Store 'img_dyn' hashmap key for the first deflayer to allow skipping it in tooltips
    pub icon_0_key: RefCell<Option<PathBuf>>,
    /// Store embedded-in-the-binary resources like icons not to load them from a file
    pub embed: nwg::EmbedResource,
    pub icon: nwg::Icon,
    decoder: nwg::ImageDecoder,
    pub window: nwg::MessageWindow,
    /// A tooltip-like (no title/resize/focus/taskbar/clickthru) window to show notifications
    /// (e.g., layer change messages)
    pub win_tt: nwg::Window,
    win_tt_ifr: nwg::ImageFrame,
    win_tt_timer: nwg::AnimationTimer,
    pub layer_notice: nwg::Notice,
    pub cfg_notice: nwg::Notice,
    pub err_notice: nwg::Notice,
    pub exit_notice: nwg::Notice,
    pub tt_notice: nwg::Notice,
    /// Receiver of error message content sent from other threads
    /// (e.g., from key event thread via WinDbgLogger that will also notify our GUI
    /// (but not pass data) after sending data to this receiver)
    pub err_recv: Option<Receiver<(String, String)>>,
    pub tt2m_channel: Option<(ASender<bool>, Receiver<bool>)>,
    // receiver will be created before a thread is spawned and moved there
    pub m2tt_sender: RefCell<Option<ASender<bool>>>,
    pub m_ptr_wh: RefCell<(u32, u32)>,
    pub tray: nwg::TrayNotification,
    pub tray_menu: nwg::Menu,
    pub tray_1cfg_m: nwg::Menu,
    pub tray_2reload: nwg::MenuItem,
    pub tray_3exit: nwg::MenuItem,
    pub img_reload: nwg::Bitmap,
    pub img_exit: nwg::Bitmap,
}
pub fn get_appdata() -> Option<PathBuf> {
    var_os("APPDATA").map(PathBuf::from)
}
pub fn get_user_home() -> Option<PathBuf> {
    var_os("USERPROFILE").map(PathBuf::from)
}
pub fn get_xdg_home() -> Option<PathBuf> {
    var_os("XDG_CONFIG_HOME").map(PathBuf::from)
}

const CFG_FD: [&str; 3] = ["", "kanata", "kanata-tray"]; // blank "" allow checking directly for
// user passed values
const ASSET_FD: [&str; 4] = ["", "icon", "img", "icons"];
const IMG_EXT: [&str; 7] = ["ico", "jpg", "jpeg", "png", "bmp", "dds", "tiff"];
const PRE_LAYER: &str = "\n🗍: "; // : invalid path marker, so should be safe to use as a separator
const TTTIMER_L: u16 = 9; // lifetime delta to duration for a tooltip timer
use crate::gui::{CFG, GUI_CFG_TX, GUI_ERR_MSG_TX, GUI_ERR_TX, GUI_EXIT_TX, GUI_TX};

pub fn send_gui_notice() {
    if let Some(gui_tx) = GUI_TX.get() {
        gui_tx.notice();
    } else {
        error!("no GUI_TX to notify GUI thread of layer changes");
    }
}
pub fn send_gui_cfg_notice() {
    if let Some(gui_tx) = GUI_CFG_TX.get() {
        gui_tx.notice();
    } else {
        error!("no GUI_CFG_TX to notify GUI thread of layer changes");
    }
}
pub fn send_gui_err_notice() {
    if let Some(gui_tx) = GUI_ERR_TX.get() {
        gui_tx.notice();
    } else {
        error!("no GUI_ERR_TX to notify GUI thread of errors");
    }
}
pub fn send_gui_exit_notice() {
    if let Some(gui_tx) = GUI_EXIT_TX.get() {
        gui_tx.notice();
    } else {
        error!("no GUI_EXIT_TX to ask GUI thread to exit");
    }
}
pub fn show_err_msg_nofail(title: String, msg: String) {
    // log gets insalized before gui, so some errors might have no target to log to, ignore them
    if let Some(gui_msg_tx) = GUI_ERR_MSG_TX.get() {
        if gui_msg_tx.send((title, msg)).is_err() {
            warn!("send_gui_err_msg_notice failed to use OS notifications")
        } else {
            // can't Error to avoid an ∞ error loop ↑
            if let Some(gui_tx) = GUI_ERR_TX.get() {
                gui_tx.notice();
            }
        }
    }
}

/// Find an icon file that matches a given config icon name for a layer `lyr_icn` or a layer name
/// `lyr_nm` (if `match_name` is `true`) or a given config icon name for the whole config `cfg_p`
/// or a config file name at various locations (where config file is, where executable is,
/// in user config folders)
fn get_icon_p<S1, S2, S3, P>(
    lyr_icn: S1,
    lyr_nm: S2,
    cfg_icn: S3,
    cfg_p: P,
    match_name: &bool,
) -> Option<String>
where
    S1: AsRef<str>,
    S2: AsRef<str>,
    S3: AsRef<str>,
    P: AsRef<Path>,
{
    get_icon_p_impl(
        lyr_icn.as_ref(),
        lyr_nm.as_ref(),
        cfg_icn.as_ref(),
        cfg_p.as_ref(),
        match_name,
    )
}

fn get_icon_p_impl(
    lyr_icn: &str,
    lyr_nm: &str,
    cfg_icn: &str,
    p: &Path,
    match_name: &bool,
) -> Option<String> {
    trace!(
        "lyr_icn={lyr_icn} lyr_nm={lyr_nm} cfg_icn={cfg_icn} cfg_p={p:?} match_name={match_name}"
    );
    let mut icon_file = PathBuf::new();
    let blank_p = Path::new("");
    let lyr_icn_p = Path::new(&lyr_icn);
    let lyr_nm_p = Path::new(&lyr_nm);
    let cfg_icn_p = Path::new(&cfg_icn);
    let cfg_stem = &p.file_stem().unwrap_or_else(|| OsStr::new(""));
    let cfg_name = &p.file_name().unwrap_or_else(|| OsStr::new(""));
    let f_name = [
        lyr_icn_p.as_os_str(),
        if *match_name {
            lyr_nm_p.as_os_str()
        } else {
            OsStr::new("")
        },
        cfg_icn_p.as_os_str(),
        cfg_stem,
        cfg_name,
    ]
    .into_iter();
    let f_ext = [
        lyr_icn_p.extension(),
        if *match_name {
            lyr_nm_p.extension()
        } else {
            None
        },
        cfg_icn_p.extension(),
        None,
        None,
    ];
    let pre_p = p.parent().unwrap_or_else(|| Path::new(""));
    let cur_exe = current_exe().unwrap_or_else(|_| PathBuf::new());
    let xdg_cfg = get_xdg_home().unwrap_or_default();
    let app_data = get_appdata().unwrap_or_default();
    let mut user_cfg = get_user_home().unwrap_or_default();
    user_cfg.push(".config");
    let parents = [
        Path::new(""),
        pre_p,
        &cur_exe,
        &xdg_cfg,
        &app_data,
        &user_cfg,
    ]; // empty path to allow no prefixes when icon path is explictily set in case it's a full
    // path already

    for (i, nm) in f_name.enumerate() {
        trace!("{}nm={:?}", "", nm);
        if nm.is_empty() {
            trace!("no file name to test, skip");
            continue;
        }
        let mut is_full_p = false;
        if nm == lyr_icn_p {
            is_full_p = true
        }; // user configs can have full paths, so test them even if all parent folders are emtpy
        if nm == cfg_icn_p {
            is_full_p = true
        };
        let icn_ext = &f_ext[i]
            .unwrap_or_else(|| OsStr::new(""))
            .to_string_lossy()
            .to_string();
        let is_icn_ext_valid = if f_ext[i].is_some() {
            if IMG_EXT.iter().any(|&i| i == icn_ext) {
                trace!("icn_ext={:?}", icn_ext);
                true
            } else {
                warn!(
                    "user icon extension \"{}\" might be invalid (or just not an extension)!",
                    icn_ext
                );
                false
            }
        } else {
            false
        };
        'p: for p_par in parents {
            trace!("{}p_par={:?}", "  ", p_par);
            if p_par == blank_p && !is_full_p {
                trace!("blank parent for non-user, skip");
                continue;
            }
            for p_kan in CFG_FD {
                trace!("{}p_kan={:?}", "    ", p_kan);
                for p_icn in ASSET_FD {
                    trace!("{}p_icn={:?}", "      ", p_icn);
                    for ext in IMG_EXT {
                        trace!("{}  ext={:?}", "        ", ext);
                        if p_par != blank_p {
                            icon_file.push(p_par);
                        } // folders
                        if !p_kan.is_empty() {
                            icon_file.push(p_kan);
                        }
                        if !p_icn.is_empty() {
                            icon_file.push(p_icn);
                        }
                        if !nm.is_empty() {
                            icon_file.push(nm);
                        }
                        if !is_full_p {
                            icon_file.set_extension(ext); // no icon name passed, iterate extensions
                        } else if !is_icn_ext_valid {
                            icon_file.add_ext(ext);
                        } else {
                            trace!("skip ext");
                        } // replace invalid icon extension
                        trace!("testing icon file {:?}", icon_file);
                        if !icon_file.is_file() {
                            icon_file.clear();
                            if p_par == blank_p && p_kan.is_empty() && p_icn.is_empty() && is_full_p
                            {
                                trace!(
                                    "skipping further sub-iters on an empty parent with user config {:?}",
                                    nm
                                );
                                continue 'p;
                            }
                        } else {
                            debug!("✓ found icon file: {}", icon_file.display().to_string());
                            return Some(icon_file.display().to_string());
                        }
                    }
                }
            }
        }
    }
    debug!("✗ no icon file found");
    None
}

pub const ICN_SZ_MENU: [u32; 2] = [24, 24]; // size for menu icons
pub const ICN_SZ_TT: [u32; 2] = [36, 36]; // size for tooltip icons
pub const ICN_SZ_MENU_I: [i32; 2] = [24, 24]; // for the builder, which needs i32
pub const ICN_SZ_TT_I: [i32; 2] = [36, 36]; // for the builder, which needs i32

macro_rules! win_ver {
    () => {{
        static WIN_VER: OnceLock<(u32, u32, u32)> = OnceLock::new();
        *WIN_VER.get_or_init(|| {
            let os_ver_i: *mut OSVERSIONINFOW = &mut OSVERSIONINFOW {
                dwOSVersionInfoSize: 0, //u32
                dwMajorVersion: 0,      //u32
                dwMinorVersion: 0,      //u32
                dwBuildNumber: 0,       //u32
                dwPlatformId: 0,        //u32
                szCSDVersion: [0; 128], //[u16; 128]
            };
            unsafe {
                if 0 == RtlGetVersion(os_ver_i) {
                    return (
                        (*os_ver_i).dwMajorVersion,
                        (*os_ver_i).dwMinorVersion,
                        (*os_ver_i).dwBuildNumber,
                    );
                }
            }
            (0, 0, 0)
        })
    }};
}
/// Convert string to wide array and append null
pub fn to_wide_str(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}
macro_rules! mouse_scale_factor {
    // screen size = dpi⋅size⋅scaleF
    () => {{
        //TODO: track changes by subscribing via RegNotifyChangeKeyValue and reset value
        static MOUSE_PTR_SCALE_F: OnceLock<u32> = OnceLock::new();
        *MOUSE_PTR_SCALE_F.get_or_init(|| {
            // 3. pointer scale factor @ Settings/Ease of Access/Mouse pointer
            let key_root = HKEY_CURRENT_USER;
            let key_path_s = r"SOFTWARE\Microsoft\Accessibility";
            let key_name_s = "CursorSize";
            let key_path = to_wide_str(key_path_s);
            let key_name = to_wide_str(key_name_s);
            let mut mouse_scale: DWORD = 0;
            let mouse_scale_p: *mut c_void = &mut mouse_scale as *mut u32 as *mut std::ffi::c_void;
            let mut mouse_scale_sz: DWORD = std::mem::size_of::<DWORD>() as DWORD;
            let res = unsafe {
                RegGetValueW(
                    key_root,
                    key_path.as_ptr(),
                    key_name.as_ptr(),
                    RRF_RT_REG_DWORD,     //restrict type to REG_DWORD
                    std::ptr::null_mut(), //pdwType
                    mouse_scale_p,
                    &mut mouse_scale_sz,
                )
            };
            match res as DWORD {
                ERROR_SUCCESS => {}
                ERROR_FILE_NOT_FOUND => {
                    log::error!(r"Registry '{}\{}' not found", key_path_s, key_name_s);
                    mouse_scale = 1;
                }
                _ => {
                    log::error!(
                        r"Registry '{}\{}' couldn't be read as DWORD {}",
                        key_path_s,
                        key_name_s,
                        res
                    );
                    mouse_scale = 1;
                }
            }
            mouse_scale
        })
    }};
}
pub fn get_mouse_ptr_size(dpi_scale: bool) -> (u32, u32) {
    // 1. get monitor DPI
    let dpi = if dpi_scale { unsafe { nwg::dpi() } } else { 96 };
    // 2. icon size @ dpi
    let cur_w = SM_CXCURSOR;
    let cur_h = SM_CYCURSOR;
    let width = unsafe { GetSystemMetricsForDpi(cur_w, dpi as u32) } as u32;
    let height = unsafe { GetSystemMetricsForDpi(cur_h, dpi as u32) } as u32;
    let mouse_scale = mouse_scale_factor!();
    (mouse_scale * width, mouse_scale * height)
}

// stores old mouse pointer position to avoid refreshing tooltips if mouse doesn't move
thread_local! {static MXY:Cell<(i32,i32)> = Cell::default();}
impl SystemTray {
    /// Read an image from a file, convert it to various formats: tray, tooltip, icon
    fn get_icon_from_file<P>(&self, ico_p: P) -> Result<Icn>
    where
        P: AsRef<str>,
    {
        self.get_icon_from_file_impl(ico_p.as_ref())
    }
    fn get_icon_from_file_impl(&self, ico_p: &str) -> Result<Icn> {
        let app_data = self.app_data.borrow();
        let icn_sz_tt = [
            app_data.gui_opts.tooltip_size.0 as u32,
            app_data.gui_opts.tooltip_size.1 as u32,
        ];
        if let Ok(img_data) = self
            .decoder
            .from_filename(ico_p)
            .and_then(|img_src| img_src.frame(0))
        {
            if let Ok(cfg_img_menu) = self.decoder.resize_image(&img_data, ICN_SZ_MENU) {
                let cfg_icon_bmp_tray = cfg_img_menu.as_bitmap()?;
                let cfg_icon_bmp_icon = cfg_icon_bmp_tray.copy_as_icon();
                if let Ok(cfg_img_menu) = self.decoder.resize_image(&img_data, icn_sz_tt) {
                    let cfg_icon_bmp_tt = cfg_img_menu.as_bitmap()?;
                    return Ok(Icn {
                        tray: cfg_icon_bmp_tray,
                        tooltip: cfg_icon_bmp_tt,
                        icon: cfg_icon_bmp_icon,
                    });
                } else {
                    debug!("✓ main ✗ icon resize Tray for {:?}", ico_p);
                }
            } else {
                debug!("✓ main ✗ icon resize TTip for {:?}", ico_p);
            }
        } else {
            debug!("✗ main 0 icon ✓ icon path for {:?}", ico_p);
        }
        bail!("✗ couldn't get a valid icon at {:?}", ico_p)
    }
    /// Read an image from a file, convert it to a menu-sized icon,
    /// assign to a menu and return the image in various formats (tray, tooltip, icon)
    fn set_menu_item_cfg_icon(
        &self,
        menu_item: &mut nwg::MenuItem,
        cfg_icon_s: &str,
        cfg_p: &PathBuf,
    ) -> Result<Icn> {
        if let Some(ico_p) = get_icon_p("", "", cfg_icon_s, cfg_p, &false) {
            if let Ok(icn) = self.get_icon_from_file(ico_p) {
                menu_item.set_bitmap(Some(&icn.tray));
                return Ok(icn);
            } else {
                debug!(
                    "✗ main 0 icon ✓ icon path, will be using DEFAULT icon for {:?}",
                    cfg_p
                );
            }
        }
        menu_item.set_bitmap(None);
        bail!("✗couldn't get a valid icon for {:?}", cfg_p)
    }
    /// Move tooltip to the current mouse pointer position
    fn update_tooltip_pos(&self) {
        let app_data = self.app_data.borrow();
        let mut x = 0;
        let mut y = 0;
        let mut is_same = false;
        MXY.with(|mxy| {
            (x, y) = nwg::GlobalCursor::position();
            let (mx, my) = mxy.get();
            if mx == x && my == y {
                is_same = true;
                return;
            }
            mxy.set((x, y));
        });
        if is_same {
            return;
        };
        let win_ver = win_ver!();
        // image width/height to take it into account when calculating overlaps
        let w = app_data.gui_opts.tooltip_size.0 as i32;
        let h = app_data.gui_opts.tooltip_size.1 as i32;
        let flags = if (win_ver.0 >= 6 && win_ver.1 >= 1) || win_ver.0 > 6 {
            TPM_WORKAREA
        } else {
            0
        };
        // 🖰 pointer size to make sure tooltip doesn't overlap,
        // don't adjust for dpi in internal calculations
        // tooltip offset vs. 🖰 pointer by 25% its size
        let (mouse_ptr_w, mouse_ptr_h) = *self.m_ptr_wh.borrow();
        let tt_off_x = (mouse_ptr_w as f64 * 0.25).round() as i32;
        let tt_off_y = (mouse_ptr_h as f64 * 0.25).round() as i32; //
        let (mouse_ptr_w, mouse_ptr_h) = (mouse_ptr_w as i32, mouse_ptr_h as i32);
        let anchorpoint = &POINT {
            x: x + tt_off_x,
            y: y + tt_off_y,
        };
        let tt_win_sz = &SIZE { cx: w, cy: h };
        let excluderect = &RECT {
            left: x.saturating_sub(mouse_ptr_w),
            right: x.saturating_add(mouse_ptr_w), // assuming ~top-left hotspot
            top: y.saturating_sub(mouse_ptr_h),
            bottom: y.saturating_add(mouse_ptr_h),
        }; //Avoid ~ mouse pointer area
        let out_rect = &mut RECT {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        };
        let ret = unsafe {
            CalculatePopupWindowPosition(anchorpoint, tt_win_sz, flags, excluderect, out_rect)
        };
        if ret != 0 {
            x = out_rect.left;
            y = out_rect.top;
        }
        let dpi = unsafe { nwg::dpi() };
        let xx = (x as f64 / (dpi as f64 / 96_f64)).round() as i32; // adjust dpi for layout
        let yy = (y as f64 / (dpi as f64 / 96_f64)).round() as i32;
        self.win_tt.set_position(xx, yy);
        // TODO: somehow still shown a bit too far off from the pointer
        if log_enabled!(Trace) {
            let (mx, my) = MXY.get();
            trace!(
                "🖰 @{mx}⋅{my} ↔{mouse_ptr_w}↕{mouse_ptr_h} (upd={}) {x}⋅{y} @ dpi={dpi} → {xx}⋅{yy} {win_ver:?} flags={flags} ex←{}→{}↑{}↓{}",
                ret != 0,
                excluderect.left,
                excluderect.right,
                excluderect.top,
                excluderect.bottom
            );
        }
    }
    /// Spawn a thread with a new 🖰 pointer watcher
    /// (that sends a signal back to GUI which in turn moves the tooltip to the new position)
    fn update_mouse_watcher(&self, tt2m_sndr: ASender<bool>, ticks: u16, poll_time: Duration) {
        debug!("   ✓   update_mouse_watcher");
        let gui_tx = self.tt_notice.sender(); // allows notifying GUI on tooltip move updates
        let (m2tt_sndr0, m2tt_rcvr) = std::sync::mpsc::channel::<bool>();
        {
            let mut m2tt_sender = self.m2tt_sender.borrow_mut();
            *m2tt_sender = Some(m2tt_sndr0.clone());
        }
        let _handler = std::thread::spawn(move || -> Result<()> {
            debug!("  ✓ Starting polling for a 🖰 pointer position");
            let mut i = 0;
            while i <= ticks {
                i += 1;
                std::thread::sleep(poll_time);
                match m2tt_rcvr.try_recv() {
                    Ok(_) => {
                        debug!("extending tooltip watcher instead of launching +1");
                        i = 0;
                    }
                    Err(TryRecvError::Empty) => {
                        trace!("send signal to reposition");
                        gui_tx.notice();
                    }
                    Err(TryRecvError::Disconnected) => {
                        debug!("internal: m2tt_sender disconnected, no more 🖰 pointer tracking");
                        break;
                    }
                }
            }
            debug!("  ✗ Stopped polling for a 🖰 pointer position");
            tt2m_sndr.send(true)?;
            Ok(())
        });
    }
    /// Show our tooltip-like notification window
    fn show_tooltip(&self, img: Option<&nwg::Bitmap>) {
        let app_data = self.app_data.borrow();
        if !app_data.gui_opts.tooltip_layer_changes {
            return;
        };
        if img.is_none() && !app_data.gui_opts.tooltip_show_blank {
            self.win_tt.set_visible(false);
            return;
        };
        static IS_INIT: OnceLock<bool> = OnceLock::new();
        if IS_INIT.get().is_none() {
            // layered win needs a special call after being initialized to appear
            let _ = IS_INIT.set(true);
            debug!("win_tt hasn't been shown as a layered window");
            let win_id = self
                .win_tt
                .handle
                .hwnd()
                .expect("win_tt should be a valid/existing window!");
            show_layered_win(win_id);
        } else {
            debug!("win_tt has been shown as a layered window");
        }
        self.win_tt_ifr.set_bitmap(img);
        {
            let mut m_ptr_wh = self.m_ptr_wh.borrow_mut();
            *m_ptr_wh = get_mouse_ptr_size(false);
        } // 🖰 pointer size so tooltip doesn't overlap
        // don't adjust for dpi in internal calculations
        self.update_tooltip_pos();
        self.win_tt.set_visible(true);
        if app_data.gui_opts.tooltip_duration != 0 {
            self.win_tt_timer.start()
        };

        if let Some((tt2m_sndr, tt2m_rcvr)) = &self.tt2m_channel {
            let mut start = false;
            match tt2m_rcvr.try_recv() {
                Ok(_) => {
                    debug!("launch a new thread");
                    start = true;
                }
                Err(TryRecvError::Empty) => {
                    if let Some(m2tt_sender) = self.m2tt_sender.borrow().as_ref() {
                        trace!("send signal to extend");
                        m2tt_sender.send(true).unwrap_or_else(|_| {
                            error!("internal: couldn't send a signal to the 🖰 pointer watcher!")
                        });
                    } else {
                        debug!(
                            "no message and no m2tt_sender_o, so no thread should be running, launch a new thread!"
                        );
                        start = true;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    error!("internal: tt2m_channel disconnected, no more 🖰 pointer tracking")
                }
            }
            let duration = 16;
            let poll_time = Duration::from_millis(duration);
            let ticks =
                (app_data.gui_opts.tooltip_duration as f64 / duration as f64).round() as u16;
            debug!(
                "will tick for {ticks} every {duration} ms to match user {}",
                app_data.gui_opts.tooltip_duration
            );
            if start {
                self.update_mouse_watcher(tt2m_sndr.clone(), ticks, poll_time);
            }
        } else {
            error!("internal: m2tt_sender doesn't exist can't track 🖰 pointer without it!");
        }
    }
    /// Hide our tooltip-like notification window
    fn hide_tooltip(&self) {
        self.win_tt.set_visible(false)
    }
    fn show_menu(&self) {
        self.update_tray_icon_cfg_group(false);
        let (x, y) = nwg::GlobalCursor::position();
        self.tray_menu.popup(x, y);
    }
    /// Add a ✓ (or highlight the icon) to the currently active config.
    /// Runs on opening of the list of configs menu
    fn update_tray_icon_cfg(
        &self,
        menu_item_cfg: &mut nwg::MenuItem,
        cfg_p: &PathBuf,
        is_active: bool,
    ) -> Result<()> {
        let mut img_dyn = self.img_dyn.borrow_mut();
        if img_dyn.contains_key(cfg_p) {
            // check if menu group icon needs to be updated to match active
            if is_active {
                if let Some(icn) = img_dyn.get(cfg_p).and_then(|maybe_icn| maybe_icn.as_ref()) {
                    self.tray_1cfg_m.set_bitmap(Some(&icn.tray))
                }
            }
        } else {
            trace!("config menu item icon missing, read config and add it (or nothing) {cfg_p:?}");
            if let Ok(cfg) = cfg::new_from_file(cfg_p) {
                if let Some(cfg_icon_s) = cfg.options.gui_opts.tray_icon {
                    debug!("loaded config without a tray icon {cfg_p:?}");
                    if let Ok(icn) = self.set_menu_item_cfg_icon(menu_item_cfg, &cfg_icon_s, cfg_p)
                    {
                        if is_active {
                            self.tray_1cfg_m.set_bitmap(Some(&icn.tray));
                        }
                        // update currently active config's icon in the combo menu
                        debug!("✓set icon {cfg_p:?}");
                        let _ = img_dyn.insert(cfg_p.clone(), Some(icn));
                    } else {
                        bail!("✗couldn't get a valid icon")
                    }
                } else {
                    bail!("✗icon not configured")
                }
            } else {
                bail!("✗couldn't load config")
            }
        }
        Ok(())
    }
    fn update_tray_icon_cfg_group(&self, force: bool) {
        if let Some(cfg) = CFG.get() {
            if let Some(k) = cfg.try_lock() {
                let idx_cfg = k.cur_cfg_idx;
                let mut tray_item_dyn = self.tray_item_dyn.borrow_mut();
                let h_cfg_i = &mut tray_item_dyn[idx_cfg];
                let is_check = h_cfg_i.checked();
                if !is_check || force {
                    let cfg_p = &k.cfg_paths[idx_cfg];
                    debug!(
                        "✗ mismatch idx_cfg={idx_cfg:?} {} {:?} cfg_p={cfg_p:?}",
                        if is_check { "✓" } else { "✗" },
                        h_cfg_i.handle
                    );
                    h_cfg_i.set_checked(true);
                    if let Err(e) = self.update_tray_icon_cfg(h_cfg_i, cfg_p, true) {
                        debug!("{e:?} {cfg_p:?}");
                        let mut img_dyn = self.img_dyn.borrow_mut();
                        img_dyn.insert(cfg_p.clone(), None);
                        self.tray_1cfg_m.set_bitmap(None); // can't update menu, so remove combo
                        // menu icon
                    };
                } else {
                    debug!("gui cfg selection matches active config");
                };
            } else {
                debug!(
                    "✗ kanata config is locked, can't get current config (likely the gui changed the layer and is still holding the lock, it will update the icon)"
                );
            }
        };
    }
    fn check_active(&self) {
        if let Some(cfg) = CFG.get() {
            let k = cfg.lock();
            let idx_cfg = k.cur_cfg_idx;
            let mut tray_item_dyn = self.tray_item_dyn.borrow_mut();
            for (i, h_cfg_i) in tray_item_dyn.iter_mut().enumerate() {
                // 1 if missing an icon, read config to get one
                let cfg_p = &k.cfg_paths[i];
                trace!("     →→→→ i={i:?} {:?} cfg_p={cfg_p:?}", h_cfg_i.handle);
                let is_active = i == idx_cfg;
                if let Err(e) = self.update_tray_icon_cfg(h_cfg_i, cfg_p, is_active) {
                    debug!("{e:?} {cfg_p:?}");
                    let mut img_dyn = self.img_dyn.borrow_mut();
                    img_dyn.insert(cfg_p.clone(), None);
                    if is_active {
                        self.tray_1cfg_m.set_bitmap(None);
                    } // update currently active config's icon in the combo menu
                };
                // 2 if wrong GUI checkmark, correct it
                if h_cfg_i.checked() && !is_active {
                    debug!("uncheck i{} act{}", i, idx_cfg);
                    h_cfg_i.set_checked(false);
                }
                if !h_cfg_i.checked() && is_active {
                    debug!("  check i{} act{}", i, idx_cfg);
                    h_cfg_i.set_checked(true);
                }
            }
        } else {
            error!("no CFG var that contains active kanata config");
        };
    }
    /// Check if tooltip data is changed, and update tooltip window size / timer duration
    fn update_tooltip_data(&self, k: &Kanata) -> bool {
        let mut app_data = self.app_data.borrow_mut();
        let mut clear = false;
        if app_data.tt_duration_pre != k.gui_opts.tooltip_duration {
            app_data.gui_opts.tooltip_duration = k.gui_opts.tooltip_duration;
            clear = true;
            app_data.tt_duration_pre = k.gui_opts.tooltip_duration;
            trace!("timer duration changed, updating");
            self.win_tt_timer.set_interval(Duration::from_millis(
                (k.gui_opts.tooltip_duration.saturating_add(1)).into(),
            ));
            self.win_tt_timer.set_lifetime(Some(Duration::from_millis(
                (k.gui_opts.tooltip_duration.saturating_add(TTTIMER_L)).into(),
            )));
        }
        if !(app_data.tt_size_pre.0 == k.gui_opts.tooltip_size.0
            && app_data.tt_size_pre.1 == k.gui_opts.tooltip_size.1)
        {
            app_data.tt_size_pre = k.gui_opts.tooltip_size;
            clear = true;
            app_data.gui_opts.tooltip_size = k.gui_opts.tooltip_size;
            trace!("tooltip_size duration changed, updating");
            let dpi = unsafe { nwg::dpi() };
            let icn_sz_tt_i = (k.gui_opts.tooltip_size.0, k.gui_opts.tooltip_size.1);
            let w = (icn_sz_tt_i.0 as f64 / (dpi as f64 / 96_f64)).round() as u32;
            let h = (icn_sz_tt_i.1 as f64 / (dpi as f64 / 96_f64)).round() as u32;
            self.win_tt.set_size(w, h);

            let icn_sz_tt_i = (
                k.gui_opts.tooltip_size.0 as u32,
                k.gui_opts.tooltip_size.1 as u32,
            );
            // todo: replace with a no-margin NWG config when it's available
            let padx = (k.gui_opts.tooltip_size.0 as f64 / 6_f64).round() as i32;
            let pady = (k.gui_opts.tooltip_size.1 as f64 / 6_f64).round() as i32;
            trace!(
                "kanata tooltip size = {icn_sz_tt_i:?}, ttsize = {w}⋅{h} offset = {padx}⋅{pady}"
            );
            self.win_tt_ifr.set_size(icn_sz_tt_i.0, icn_sz_tt_i.1);
            self.win_tt_ifr.set_position(-padx, -pady);
        }
        clear
    }
    /// Reload config file, currently active (`i=None`) or matching a given `i` index
    fn reload_cfg(&self, i: Option<usize>) -> Result<()> {
        use nwg::TrayNotificationFlags as f_tray;
        let mut msg_title = "".to_string();
        let mut msg_content = "".to_string();
        let mut flags = f_tray::empty();
        if let Some(cfg) = CFG.get() {
            let mut k = cfg.lock();
            let paths = &k.cfg_paths;
            let idx_cfg = match i {
                Some(idx) => {
                    if idx < paths.len() {
                        idx
                    } else {
                        error!(
                            "Invalid config index {} while kanata has only {} configs loaded",
                            idx + 1,
                            paths.len()
                        );
                        k.cur_cfg_idx
                    }
                }
                None => k.cur_cfg_idx,
            };
            let path_cur = &paths[idx_cfg];
            let path_cur_s = path_cur.display().to_string();
            let path_cur_cc = path_cur.clone();
            msg_content += &path_cur_s;
            let cfg_name = &path_cur
                .file_name()
                .unwrap_or_else(|| OsStr::new(""))
                .to_string_lossy()
                .to_string();
            if log_enabled!(Debug) {
                let cfg_icon = &k.gui_opts.tray_icon;
                let cfg_icon_s = cfg_icon.clone().unwrap_or("✗".to_string());
                let layer_id = k.layout.b().current_layer();
                let layer_name = &k.layer_info[layer_id].name;
                let layer_icon = &k.layer_info[layer_id].icon;
                let layer_icon_s = layer_icon.clone().unwrap_or("✗".to_string());
                debug!(
                    "pre reload tray_icon={} layer_name={} layer_icon={}",
                    cfg_icon_s, layer_name, layer_icon_s
                );
            }
            match i {
                Some(idx) => {
                    if let Ok(()) = k.live_reload_n(idx) {
                        msg_title += &("🔄 \"".to_owned() + cfg_name + "\" loaded");
                        flags |= f_tray::USER_ICON;
                    } else {
                        msg_title += &("🔄 \"".to_owned() + cfg_name + "\" NOT loaded");
                        flags |= f_tray::ERROR_ICON | f_tray::LARGE_ICON;
                        {
                            let app_data = self.app_data.borrow();
                            if app_data.gui_opts.notify_cfg_reload_silent {
                                flags |= f_tray::SILENT;
                            }
                        }
                        self.tray.show(
                            &msg_content,
                            Some(&msg_title),
                            Some(flags),
                            Some(&self.icon),
                        );
                        bail!("{msg_content}");
                    }
                }
                None => {
                    if let Ok(()) = k.live_reload() {
                        msg_title += &("🔄 \"".to_owned() + cfg_name + "\" reloaded");
                        flags |= f_tray::USER_ICON;
                    } else {
                        msg_title += &("🔄 \"".to_owned() + cfg_name + "\" NOT reloaded");
                        flags |= f_tray::ERROR_ICON | f_tray::LARGE_ICON;
                        {
                            let app_data = self.app_data.borrow();
                            if app_data.gui_opts.notify_cfg_reload_silent {
                                flags |= f_tray::SILENT;
                            }
                        }
                        self.tray.show(
                            &msg_content,
                            Some(&msg_title),
                            Some(flags),
                            Some(&self.icon),
                        );
                        bail!("{msg_content}");
                    }
                }
            };
            let cfg_icon = &k.gui_opts.tray_icon;
            let layer_id = k.layout.b().current_layer();
            let layer_name = &k.layer_info[layer_id].name;
            let layer_icon = &k.layer_info[layer_id].icon;
            let mut cfg_layer_pkey = PathBuf::new(); // path key
            cfg_layer_pkey.push(path_cur_cc.clone());
            cfg_layer_pkey.push(PRE_LAYER.to_owned() + layer_name); //:invalid path marker,
            // so should be safe to use as
            // a separator
            let cfg_layer_pkey_s = cfg_layer_pkey.display().to_string();
            if log_enabled!(Debug) {
                let layer_icon_s = layer_icon.clone().unwrap_or("✗".to_string());
                debug!(
                    "pos reload tray_icon={:?} layer_name={:?} layer_icon={:?}",
                    cfg_icon, layer_name, layer_icon_s
                );
            }

            let _ = self.update_tooltip_data(&k); // check for changes before they're overwritten ↓
            {
                *self.app_data.borrow_mut() = update_app_data(&k)?;
            }
            self.tray.set_tip(&cfg_layer_pkey_s); // update tooltip to point to the newer config
            let clear = i.is_none();
            self.update_tray_icon(
                cfg_layer_pkey,
                &cfg_layer_pkey_s,
                layer_name,
                layer_icon,
                path_cur_cc,
                clear,
            )
        } else {
            msg_title += "✗ Config NOT reloaded, no CFG";
            warn!("{}", msg_title);
            flags |= f_tray::ERROR_ICON;
        };
        flags |= f_tray::LARGE_ICON; // todo: fails without this, must have SM_CXICON x SM_CYICON?
        {
            let app_data = self.app_data.borrow();
            if app_data.gui_opts.notify_cfg_reload_silent {
                flags |= f_tray::SILENT;
            }
        }
        self.tray.show(
            &msg_content,
            Some(&msg_title),
            Some(flags),
            Some(&self.icon),
        );
        Ok(())
    }
    fn reload_layer_icon(&self) {
        let _ = self.reload_cfg_or_layer_icon(false);
    }
    /// Show OS notification message with an error coming from WinDbgLogger
    fn notify_error(&self) {
        let app_data = self.app_data.borrow();
        if !app_data.gui_opts.notify_error {
            return;
        };
        use nwg::TrayNotificationFlags as f_tray;
        let mut msg_title = "".to_string();
        let mut msg_content = "".to_string();
        let mut flags = f_tray::empty();
        if let Some(gui_msg_rx) = &self.err_recv {
            match gui_msg_rx.try_recv() {
                Ok((title, msg)) => {
                    msg_title += &title;
                    msg_content += &msg;
                }
                Err(TryRecvError::Empty) => {
                    msg_title += "internal";
                    msg_content += "channel to receive errors is Empty";
                }
                Err(TryRecvError::Disconnected) => {
                    msg_title += "internal";
                    msg_content += "channel to receive errors is Disconnected";
                }
            }
        } else {
            msg_title += "internal";
            msg_content += "SystemTray is supposed to have a valid 'err_recv' field value"
        }
        flags |= f_tray::ERROR_ICON;
        if app_data.gui_opts.notify_cfg_reload_silent {
            flags |= f_tray::SILENT;
        }
        let msg_title = strip_ansi_escapes::strip_str(&msg_title);
        let msg_content = strip_ansi_escapes::strip_str(&msg_content);
        self.tray.show(
            &msg_content,
            Some(&msg_title),
            Some(flags),
            Some(&self.icon),
        );
    }
    /// Update tray icon data on config reload
    fn reload_cfg_icon(&self) {
        let _ = self.reload_cfg_or_layer_icon(true);
    }
    /// Update tray icon data on layer change (and config reload)
    fn reload_cfg_or_layer_icon(&self, is_cfg: bool) -> Result<()> {
        if let Some(cfg) = CFG.get() {
            if let Some(k) = cfg.try_lock() {
                let paths = &k.cfg_paths;
                let idx_cfg = k.cur_cfg_idx;
                let path_cur = &paths[idx_cfg];
                let path_cur_cc = path_cur.clone();
                let cfg_icon = &k.gui_opts.tray_icon;
                let layer_id = k.layout.b().current_layer();
                let layer_name = &k.layer_info[layer_id].name;
                let layer_icon = &k.layer_info[layer_id].icon;

                let mut cfg_layer_pkey = PathBuf::new(); // path key
                cfg_layer_pkey.push(path_cur_cc.clone());
                cfg_layer_pkey.push(PRE_LAYER.to_owned() + layer_name); //:invalid path marker,
                // so should be safe
                // to use as a separator
                let cfg_layer_pkey_s = cfg_layer_pkey.display().to_string();
                if log_enabled!(Debug) {
                    let cfg_name = &path_cur
                        .file_name()
                        .unwrap_or_else(|| OsStr::new(""))
                        .to_string_lossy()
                        .to_string();
                    let cfg_icon_s = layer_icon.clone().unwrap_or("✗".to_string());
                    let layer_icon_s = cfg_icon.clone().unwrap_or("✗".to_string());
                    debug!(
                        "✓ layer changed to ‘{}’ with icon ‘{}’ @ ‘{}’ tray_icon ‘{}’",
                        layer_name, layer_icon_s, cfg_name, cfg_icon_s
                    );
                }

                let clear = self.update_tooltip_data(&k);
                if is_cfg {
                    *self.app_data.borrow_mut() = update_app_data(&k)?;
                }
                if is_cfg {
                    let app_data = self.app_data.borrow();
                    if app_data.gui_opts.notify_cfg_reload {
                        use nwg::TrayNotificationFlags as f_tray;
                        let cfg_name = &path_cur
                            .file_name()
                            .unwrap_or_else(|| OsStr::new(""))
                            .to_string_lossy()
                            .to_string();
                        let msg_title = "🔄 \"".to_owned() + cfg_name + "\" re-loaded";
                        let msg_content = &path_cur.display().to_string();
                        let mut flags = f_tray::empty() | f_tray::USER_ICON | f_tray::LARGE_ICON;
                        if app_data.gui_opts.notify_cfg_reload_silent {
                            flags |= f_tray::SILENT;
                        }
                        self.tray.show(
                            msg_content,
                            Some(&msg_title),
                            Some(flags),
                            Some(&self.icon),
                        );
                    }
                }
                self.tray.set_tip(&cfg_layer_pkey_s);
                self.update_tray_icon(
                    cfg_layer_pkey,
                    &cfg_layer_pkey_s,
                    layer_name,
                    layer_icon,
                    path_cur_cc,
                    clear,
                )
            } else {
                debug!(
                    "✗ kanata config is locked, can't get current layer (likely the gui changed the layer and is still holding the lock, it will update the icon)"
                );
            }
        } else {
            warn!("✗ Layer indicator NOT changed, no CFG");
        };
        Ok(())
    }
    /// Update tray icon data given various config/layer info
    /// * `cfg_layer_pkey` - "path␤🗍: layer_name" unique icon id
    /// * `path_cur_cc` - "path" without the layer name
    /// * `clear` - reset stored icon cached paths/files
    fn update_tray_icon(
        &self,
        cfg_layer_pkey: PathBuf,
        cfg_layer_pkey_s: &str,
        layer_name: &str,
        layer_icon: &Option<String>,
        path_cur_cc: PathBuf,
        clear: bool,
    ) {
        let mut img_dyn = self.img_dyn.borrow_mut(); // update the tray icons
        let mut icon_act_key = self.icon_act_key.borrow_mut(); // update the tray icon active path
        let mut icon_0_key = self.icon_0_key.borrow_mut(); // update the tray tooltip layer0 path
        if clear {
            *img_dyn = Default::default();
            *icon_act_key = Default::default();
            *icon_0_key = Some(cfg_layer_pkey.clone());
            debug!("reloading active config, clearing img_dyn/_active cache");
        }
        let app_data = self.app_data.borrow();
        let skip_tt = app_data.gui_opts.tooltip_no_base
            && icon_0_key
                .as_ref()
                .filter(|p| **p == cfg_layer_pkey)
                .is_some();
        if icon_0_key.is_none() {
            warn!("internal bug?: icon_0_key should never be empty?")
        }
        if let Some(icn_opt) = img_dyn.get(&cfg_layer_pkey) {
            // 1a config+layer path has already been checked
            if let Some(icn) = icn_opt {
                self.tray.set_icon(&icn.icon);
                *icon_act_key = Some(cfg_layer_pkey);
                if !skip_tt {
                    self.show_tooltip(Some(&icn.tooltip));
                }
            } else {
                info!(
                    "no icon found, using default for config+layer = {}",
                    cfg_layer_pkey_s
                );
                self.tray.set_icon(&self.icon);
                *icon_act_key = Some(cfg_layer_pkey);
                self.show_tooltip(None);
            }
        } else if layer_icon.is_some() || app_data.gui_opts.icon_match_layer_name {
            let layer_icon = match layer_icon {
                Some(layer_icon_inner) => {
                    trace!("configured layer icon = {}", layer_icon_inner);
                    layer_icon_inner
                }
                None => {
                    trace!(
                        "no configured layer icon, checking its name = {}",
                        layer_name
                    );
                    layer_name
                }
            };
            // 1b cfg+layer path hasn't been checked, but layer has an icon configured...
            // or configured to check its name, so check it
            if let Some(ico_p) = get_icon_p(
                layer_icon,
                layer_name,
                "",
                &path_cur_cc,
                &app_data.gui_opts.icon_match_layer_name,
            ) {
                if let Ok(icn) = self.get_icon_from_file(ico_p) {
                    info!(
                        "✓ Using an icon from this config+layer: {}",
                        cfg_layer_pkey_s
                    );
                    self.tray.set_icon(&icn.icon);
                    if !skip_tt {
                        self.show_tooltip(Some(&icn.tooltip));
                    }
                    let _ = img_dyn.insert(cfg_layer_pkey.clone(), Some(icn));
                    *icon_act_key = Some(cfg_layer_pkey);
                } else {
                    warn!(
                        "✗ Invalid icon file \"{layer_icon}\" from this config+layer: {}",
                        cfg_layer_pkey_s
                    );
                    let _ = img_dyn.insert(cfg_layer_pkey.clone(), None);
                    self.tray.set_icon(&self.icon);
                    *icon_act_key = Some(cfg_layer_pkey);
                    self.show_tooltip(None);
                }
            } else {
                warn!(
                    "✗ Invalid icon path \"{layer_icon}\" from this config+layer: {}",
                    cfg_layer_pkey_s
                );
                let _ = img_dyn.insert(cfg_layer_pkey.clone(), None);
                self.tray.set_icon(&self.icon);
                *icon_act_key = Some(cfg_layer_pkey);
                self.show_tooltip(None);
            }
        } else if img_dyn.contains_key(&path_cur_cc) {
            // 2a no layer icon configured, but config icon exists, use it
            if let Some(icn) = img_dyn.get(&path_cur_cc).unwrap() {
                self.tray.set_icon(&icn.icon);
                *icon_act_key = Some(path_cur_cc);
                self.show_tooltip(None);
            } else {
                info!(
                    "no icon found, using default for config: {}",
                    path_cur_cc.display().to_string()
                );
                self.tray.set_icon(&self.icon);
                *icon_act_key = Some(path_cur_cc);
                self.show_tooltip(None);
            }
        } else {
            // 2b no layer icon configured, no config icon, use config path
            let cfg_icon_p = if let Some(cfg_icon) = &app_data.cfg_icon {
                cfg_icon
            } else {
                ""
            };
            if let Some(ico_p) = get_icon_p(
                "",
                layer_name,
                cfg_icon_p,
                &path_cur_cc,
                &app_data.gui_opts.icon_match_layer_name,
            ) {
                if let Ok(icn) = self.get_icon_from_file(ico_p) {
                    info!(
                        "✓ Using an icon from this config: {}",
                        path_cur_cc.display().to_string()
                    );
                    self.tray.set_icon(&icn.icon);
                    if !skip_tt {
                        self.show_tooltip(Some(&icn.tooltip));
                    }
                    let _ = img_dyn.insert(cfg_layer_pkey.clone(), Some(icn));
                    *icon_act_key = Some(cfg_layer_pkey);
                } else {
                    warn!(
                        "✗ Invalid icon file \"{cfg_icon_p}\" from this config: {}",
                        cfg_layer_pkey.display().to_string()
                    );
                    let _ = img_dyn.insert(cfg_layer_pkey.clone(), None);
                    *icon_act_key = Some(cfg_layer_pkey);
                    self.tray.set_icon(&self.icon);
                    self.show_tooltip(None);
                }
            } else {
                warn!(
                    "✗ Invalid icon path \"{cfg_icon_p}\" from this config: {}",
                    cfg_layer_pkey.display().to_string()
                );
                let _ = img_dyn.insert(cfg_layer_pkey.clone(), None);
                *icon_act_key = Some(cfg_layer_pkey);
                self.tray.set_icon(&self.icon);
                self.show_tooltip(None);
            }
        }
    }
    fn exit(&self) {
        let handlers = self.handlers_dyn.borrow();
        for handler in handlers.iter() {
            nwg::unbind_event_handler(handler);
        }
        nwg::stop_thread_dispatch();
    }

    fn build_win_tt(&self) -> Result<nwg::Window, nwg::NwgError> {
        let f_style = wf::POPUP;
        let f_ex = WS_CLICK_THRU
       | WS_EX_NOACTIVATE   //0x8000000L top-level win doesn't become foreground win on user click
       | WS_EX_TOOLWINDOW   // remove from the taskbar (floating toolbar)
       ;

        let mut window: nwg::Window = Default::default();
        let dpi = unsafe { nwg::dpi() };
        let app_data = self.app_data.borrow();
        let icn_sz_tt_i = (
            app_data.gui_opts.tooltip_size.0,
            app_data.gui_opts.tooltip_size.1,
        );
        let w = (icn_sz_tt_i.0 as f64 / (dpi as f64 / 96_f64)).round() as i32;
        let h = (icn_sz_tt_i.1 as f64 / (dpi as f64 / 96_f64)).round() as i32;
        trace!("Active Kanata Layer win size = {w}⋅{h}");
        nwg::Window::builder()
            .title("Active Kanata Layer")
            .size((w, h))
            .position((0, 0))
            .center(false)
            .topmost(true)
            .maximized(false)
            .minimized(false)
            .flags(f_style)
            .ex_flags(f_ex)
            .icon(None)
            .accept_files(false)
            .build(&mut window)?;
        Ok(window)
    }
}

pub mod system_tray_ui {
    use super::*;
    use native_windows_gui::{self as nwg, MousePressEvent};
    use std::cell::RefCell;
    use std::ops::Deref;
    use std::rc::Rc;
    use windows_sys::Win32::UI::Shell::SIID_DELETE;

    pub struct SystemTrayUi {
        inner: Rc<SystemTray>,
        handler_def: RefCell<Vec<nwg::EventHandler>>,
    }

    impl nwg::NativeUi<SystemTrayUi> for SystemTray {
        fn build_ui(mut d: SystemTray) -> Result<SystemTrayUi, nwg::NwgError> {
            use nwg::Event as E;

            let app_data = d.app_data.borrow().clone();
            d.tray_item_dyn = RefCell::new(Default::default());
            d.handlers_dyn = RefCell::new(Default::default());
            d.decoder = Default::default();
            nwg::ImageDecoder::builder().build(&mut d.decoder)?;
            // Resources
            d.embed = Default::default();
            d.embed = nwg::EmbedResource::load(None)?;
            nwg::Icon::builder()
                .source_embed(Some(&d.embed))
                .source_embed_str(Some("iconMain"))
                .strict(true) /*use sys, not panic, if missing*/
                .build(&mut d.icon)?;

            let (sndr, rcvr) = std::sync::mpsc::channel();
            d.tt2m_channel = Some((sndr, rcvr));
            let (sndr, rcvr) = std::sync::mpsc::channel();
            d.err_recv = Some(rcvr);
            if GUI_ERR_MSG_TX.set(sndr).is_err() {
                warn!("Someone else set our ‘GUI_ERR_MSG_TX’");
            };

            // Controls
            nwg::MessageWindow::builder().build(&mut d.window)?;
            nwg::Notice::builder()
                .parent(&d.window)
                .build(&mut d.layer_notice)?;
            nwg::Notice::builder()
                .parent(&d.window)
                .build(&mut d.cfg_notice)?;
            nwg::Menu::builder()
                .parent(&d.window)
                .popup(true) /*context menu*/	//
                .build(&mut d.tray_menu)?;
            nwg::Notice::builder()
                .parent(&d.window)
                .build(&mut d.tt_notice)?;
            nwg::Notice::builder()
                .parent(&d.window)
                .build(&mut d.err_notice)?;
            nwg::Notice::builder()
                .parent(&d.window)
                .build(&mut d.exit_notice)?;
            nwg::Menu::builder()
                .parent(&d.tray_menu)
                .text("&F Load config") //
                .build(&mut d.tray_1cfg_m)?;
            nwg::MenuItem::builder()
                .parent(&d.tray_menu)
                .text("&R Reload config") //
                .build(&mut d.tray_2reload)?;
            nwg::MenuItem::builder()
                .parent(&d.tray_menu)
                .text("&X Exit\t‹⎈␠⎋") //
                .build(&mut d.tray_3exit)?;

            if app_data.gui_opts.tooltip_layer_changes {
                d.win_tt = d.build_win_tt().expect("Tooltip window");
                nwg::AnimationTimer::builder()
                    .parent(&d.window)
                    .interval(Duration::from_millis(
                        (app_data.gui_opts.tooltip_duration.saturating_add(1)).into(),
                    ))
                    .lifetime(Some(Duration::from_millis(
                        (app_data.gui_opts.tooltip_duration + TTTIMER_L).into(),
                    )))
                    .max_tick(None)
                    .active(false)
                    .build(&mut d.win_tt_timer)?;

                let icn_sz_tt_i = (
                    app_data.gui_opts.tooltip_size.0 as i32,
                    app_data.gui_opts.tooltip_size.1 as i32,
                );
                // todo: replace with a no-margin NWG config when it's available
                let padx = (app_data.gui_opts.tooltip_size.0 as f64 / 6_f64).round() as i32;
                let pady = (app_data.gui_opts.tooltip_size.1 as f64 / 6_f64).round() as i32;
                let pad = (-padx, -pady);
                trace!("kanata tooltip size = {icn_sz_tt_i:?}, offset = {padx}⋅{pady}");
                let mut cfg_icon_bmp_tray = Default::default();
                nwg::Bitmap::builder()
                    .source_embed(Some(&d.embed))
                    .source_embed_str(Some("imgMain"))
                    .strict(true)
                    .size(Some(ICN_SZ_MENU.into()))
                    .build(&mut cfg_icon_bmp_tray)?;
                nwg::ImageFrame::builder()
                    .parent(&d.win_tt)
                    .size(icn_sz_tt_i)
                    .position(pad)
                    .build(&mut d.win_tt_ifr)?;
            }

            let mut tmp_bitmap = Default::default();
            nwg::Bitmap::builder()
                .source_embed(Some(&d.embed))
                .source_embed_str(Some("imgReload"))
                .strict(true)
                .size(Some(ICN_SZ_MENU.into()))
                .build(&mut tmp_bitmap)?;
            let img_exit = nwg::Bitmap::from_system_icon(SIID_DELETE);
            d.tray_2reload.set_bitmap(Some(&tmp_bitmap));
            d.tray_3exit.set_bitmap(Some(&img_exit));
            d.img_reload = tmp_bitmap;
            d.img_exit = img_exit;

            let mut main_tray_icon_l = Default::default();
            let mut main_tray_icon_is = false;
            {
                let mut tray_item_dyn = d.tray_item_dyn.borrow_mut();
                let mut img_dyn = d.img_dyn.borrow_mut();
                let mut icon_act_key = d.icon_act_key.borrow_mut();
                let mut icon_0_key = d.icon_0_key.borrow_mut();
                const MENU_ACC: &str = "1234567890ASDFGQWERTZXCVBYUIOPHJKLNM";
                const M_E: usize = MENU_ACC.len() - 1;
                let layer0_icon_s = &app_data.layer0_icon.clone().unwrap_or("".to_string());
                let cfg_icon_s = &app_data.cfg_icon.clone().unwrap_or("".to_string());
                if !(app_data.cfg_p).is_empty() {
                    for (i, cfg_p) in app_data.cfg_p.iter().enumerate() {
                        let i_acc = match i {
                            // accelerators from 1–0, A–Z starting from home row for easier presses
                            0..=M_E => format!("&{} ", &MENU_ACC[i..i + 1]),
                            _ => "  ".to_string(),
                        };
                        let cfg_name = &cfg_p
                            .file_name()
                            .unwrap_or_else(|| OsStr::new(""))
                            .to_string_lossy()
                            .to_string(); //kanata.kbd
                        let menu_text = format!("{cfg_name}\t{i_acc}"); // kanata.kbd &1
                        let mut menu_item = Default::default();
                        if i == 0 {
                            nwg::MenuItem::builder()
                                .parent(&d.tray_1cfg_m)
                                .text(&menu_text)
                                .check(true)
                                .build(&mut menu_item)?;
                        } else {
                            nwg::MenuItem::builder()
                                .parent(&d.tray_1cfg_m)
                                .text(&menu_text)
                                .build(&mut menu_item)?;
                        }
                        if i == 0 {
                            // add icons if exists, hashed by config path
                            // (for active config, others will create on load)
                            if let Some(ico_p) = get_icon_p(
                                layer0_icon_s,
                                &app_data.layer0_name,
                                cfg_icon_s,
                                cfg_p,
                                &app_data.gui_opts.icon_match_layer_name,
                            ) {
                                let mut cfg_layer_pkey = PathBuf::new(); // path key
                                cfg_layer_pkey.push(cfg_p.clone());
                                cfg_layer_pkey.push(PRE_LAYER.to_owned() + &app_data.layer0_name);
                                let cfg_layer_pkey_s = cfg_layer_pkey.display().to_string();
                                *icon_0_key = Some(cfg_layer_pkey.clone());
                                if let Ok(icn) = d.get_icon_from_file(&ico_p) {
                                    debug!("✓ main 0 config: using icon for {}", cfg_layer_pkey_s);
                                    main_tray_icon_l = icn.tray.copy_as_icon();
                                    main_tray_icon_is = true;
                                    let _ = img_dyn.insert(cfg_layer_pkey, Some(icn));
                                } else {
                                    info!(
                                        "✗ main 0 icon ✓ icon path, will be using DEFAULT icon for {:?}",
                                        cfg_p
                                    );
                                    let _ = img_dyn.insert(cfg_layer_pkey, None);
                                }
                            } else {
                                debug!("✗ main 0 config: using DEFAULT icon for {:?}", cfg_p);
                                let mut cfg_icon_bmp_tray = Default::default();
                                let mut cfg_icon_bmp_tt = Default::default();
                                let mut cfg_icon_bmp_icon = Default::default();
                                nwg::Bitmap::builder()
                                    .source_embed(Some(&d.embed))
                                    .source_embed_str(Some("imgMain"))
                                    .strict(true)
                                    .size(Some(ICN_SZ_MENU.into()))
                                    .build(&mut cfg_icon_bmp_tray)?;
                                nwg::Bitmap::builder()
                                    .source_embed(Some(&d.embed))
                                    .source_embed_str(Some("imgMain"))
                                    .strict(true)
                                    .size(Some(ICN_SZ_TT.into()))
                                    .build(&mut cfg_icon_bmp_tt)?;
                                nwg::Icon::builder()
                                    .source_embed(Some(&d.embed))
                                    .source_embed_str(Some("iconMain"))
                                    .strict(true)
                                    .build(&mut cfg_icon_bmp_icon)?;
                                let _ = img_dyn.insert(
                                    cfg_p.clone(),
                                    Some(Icn {
                                        tray: cfg_icon_bmp_tray,
                                        tooltip: cfg_icon_bmp_tt,
                                        icon: cfg_icon_bmp_icon,
                                    }),
                                );
                                *icon_act_key = Some(cfg_p.clone());
                            }
                            // Set tray menu config item icons, ignores layers since these
                            // are per config
                            if let Ok(icn) =
                                d.set_menu_item_cfg_icon(&mut menu_item, cfg_icon_s, cfg_p)
                            {
                                // show currently active config's icon in the combo menu
                                d.tray_1cfg_m.set_bitmap(Some(&icn.tray));
                                let _ = img_dyn.insert(cfg_p.clone(), Some(icn));
                            } else {
                                let _ = img_dyn.insert(cfg_p.clone(), None);
                            }
                        }
                        tray_item_dyn.push(menu_item);
                    }
                } else {
                    warn!("Didn't get any config paths from Kanata!")
                }
            }
            let main_tray_icon = match main_tray_icon_is {
                true => Some(&main_tray_icon_l),
                false => Some(&d.icon),
            };
            nwg::TrayNotification::builder()
                .parent(&d.window)
                .icon(main_tray_icon)
                .tip(Some(&app_data.tooltip))
                .build(&mut d.tray)?;

            let ui = SystemTrayUi {
                // Wrap-up
                inner: Rc::new(d),
                handler_def: Default::default(),
            };

            let evt_ui = Rc::downgrade(&ui.inner); // Events
            let handle_events = move |evt, _evt_data, handle| {
                if let Some(evt_ui) = evt_ui.upgrade() {
                    match evt {
                        E::OnNotice =>
                            if handle == evt_ui.layer_notice {
                                SystemTray::reload_layer_icon(&evt_ui);
                            } else if handle == evt_ui.cfg_notice {
                                SystemTray::reload_cfg_icon(&evt_ui);
                            } else if handle == evt_ui.err_notice {
                                SystemTray::notify_error(&evt_ui);
                            } else if handle == evt_ui.exit_notice {
                                SystemTray::exit(&evt_ui);
                            } else if handle == evt_ui.tt_notice {
                                SystemTray::update_tooltip_pos(&evt_ui);}
                        E::OnWindowClose =>
                            if handle == evt_ui.window {SystemTray::exit  (&evt_ui);}
                        E::OnMousePress(MousePressEvent::MousePressLeftUp) =>
                            if handle == evt_ui.tray {SystemTray::show_menu(&evt_ui);}
                        E::OnContextMenu/*🖰›*/ =>
                            if handle == evt_ui.tray {SystemTray::show_menu(&evt_ui);}
                        E::OnTimerStop/*🕐*/ => {SystemTray::hide_tooltip(&evt_ui);}
                        E::OnMenuHover =>
                            if        handle == evt_ui.tray_1cfg_m {
                                SystemTray::check_active(&evt_ui);}
                        E::OnMenuItemSelected =>
                            if        handle == evt_ui.tray_2reload   {
                            let _ = SystemTray::reload_cfg(&evt_ui,None);
                            SystemTray::update_tray_icon_cfg_group(&evt_ui,true);
                        } else if handle == evt_ui.tray_3exit     {SystemTray::exit  (&evt_ui);
                        } else if let
                            ControlHandle::MenuItem(_parent, _id) = handle {
                              {let tray_item_dyn    = &evt_ui.tray_item_dyn.borrow(); //
                              for (i, h_cfg) in tray_item_dyn.iter().enumerate() {
                                if &handle == h_cfg {
                                    for h_cfg_j in tray_item_dyn.iter() {
                                      if h_cfg_j.checked() {h_cfg_j.set_checked(false);} }
                                      // uncheck others
                                    h_cfg.set_checked(true); // check self
                                  let _ = SystemTray::reload_cfg(&evt_ui,Some(i)); // depends
                                }
                              }
                            }
                          }
                      _ => {}
                    }
                }
            };
            ui.handler_def
                .borrow_mut()
                .push(nwg::full_bind_event_handler(
                    &ui.window.handle,
                    handle_events,
                ));
            Ok(ui)
        }
    }

    impl Drop for SystemTrayUi {
        /// To make sure that everything is freed without issues, the default handler
        /// must be unbound.
        fn drop(&mut self) {
            let mut handlers = self.handler_def.borrow_mut();
            for handler in handlers.drain(0..) {
                nwg::unbind_event_handler(&handler);
            }
        }
    }
    impl Deref for SystemTrayUi {
        type Target = SystemTray;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
}
use winapi::um::winuser::{
    SetLayeredWindowAttributes, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT,
};
pub const WS_CLICK_THRU: u32 = WS_EX_LAYERED | WS_EX_TRANSPARENT;

use nwg::WindowFlags as wf;
/// Build a tooltip-like window to notify of user events
fn show_layered_win(win_id: HWND) {
    use winapi::um::wingdi::RGB;
    use winapi::um::winuser::LWA_ALPHA;
    let cr_key: COLORREF = RGB(0, 0, 0);
    let b_alpha: BYTE = 255;
    let dw_flags: DWORD = LWA_ALPHA;
    unsafe {
        SetLayeredWindowAttributes(win_id, cr_key, b_alpha, dw_flags);
    } // layered window doesn't appear w/o this call
}

pub fn update_app_data(k: &MutexGuard<Kanata>) -> Result<SystemTrayData> {
    let paths = &k.cfg_paths;
    let path_cur = &paths[0];
    let layer0_id = k.layout.b().current_layer();
    let layer0_name = &k.layer_info[layer0_id].name;
    let layer0_icon = &k.layer_info[layer0_id].icon;
    Ok(SystemTrayData {
        tooltip: path_cur.display().to_string(),
        cfg_p: paths.clone(),
        cfg_icon: k.gui_opts.tray_icon.clone(),
        layer0_name: layer0_name.clone(),
        layer0_icon: layer0_icon.clone(),
        gui_opts: k.gui_opts.clone(),
        tt_duration_pre: k.gui_opts.tooltip_duration,
        tt_size_pre: k.gui_opts.tooltip_size,
    })
}
pub fn build_tray(cfg: &Arc<Mutex<Kanata>>) -> Result<system_tray_ui::SystemTrayUi> {
    let k = cfg.lock();
    let app_data = update_app_data(&k)?;
    let app = SystemTray {
        app_data: RefCell::new(app_data),
        ..Default::default()
    };
    Ok(SystemTray::build_ui(app)?)
}

pub use log::*;
pub use std::io::{IsTerminal, stdout};
pub use winapi::shared::minwindef::BOOL;
pub use winapi::um::wincon::{ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole};

use once_cell::sync::Lazy;
pub static IS_TERM: Lazy<bool> = Lazy::new(|| stdout().is_terminal());
pub static IS_CONSOLE: Lazy<bool> =
    Lazy::new(|| unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0i32 });
