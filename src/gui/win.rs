use crate::Kanata;
use anyhow::{bail, Result};
use core::cell::RefCell;
use log::Level::*;

use native_windows_gui as nwg;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::env::{current_exe, var_os};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

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
    pub icon_match_layer_name: bool,
}
#[derive(Default)]
pub struct SystemTray {
    pub app_data: RefCell<SystemTrayData>,
    /// Store dynamically created tray menu items
    pub tray_item_dyn: RefCell<Vec<nwg::MenuItem>>,
    /// Store dynamically created tray menu items' handlers
    pub handlers_dyn: RefCell<Vec<nwg::EventHandler>>,
    /// Store dynamically created icons to not load them from a file every time
    pub icon_dyn: RefCell<HashMap<PathBuf, Option<nwg::Icon>>>,
    /// Store dynamically created icons to not load them from a file every time
    /// (bitmap format needed to set MenuItem's icons)
    pub img_dyn: RefCell<HashMap<PathBuf, Option<nwg::Bitmap>>>,
    /// Store 'icon_dyn' hashmap key for the currently active icon ('cfg_path:layer_name' format)
    pub icon_active: RefCell<Option<PathBuf>>,
    /// Store embedded-in-the-binary resources like icons not to load them from a file
    pub embed: nwg::EmbedResource,
    pub icon: nwg::Icon,
    pub window: nwg::MessageWindow,
    pub layer_notice: nwg::Notice,
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
use crate::gui::{CFG, GUI_TX};

pub fn send_gui_notice() {
    if let Some(gui_tx) = GUI_TX.get() {
        gui_tx.notice();
    } else {
        error!("no GUI_TX to notify GUI thread of layer changes");
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
        let is_icn_ext_valid = if !IMG_EXT.iter().any(|&i| i == icn_ext) && f_ext[i].is_some() {
            warn!(
                "user icon extension \"{}\" might be invalid (or just not an extension)!",
                icn_ext
            );
            false
        } else {
            trace!("icn_ext={:?}", icn_ext);
            true
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
                                trace!("skipping further sub-iters on an empty parent with user config {:?}",nm);
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

fn set_menu_item_cfg_icon(
    menu_item: &mut nwg::MenuItem,
    cfg_icon_s: &str,
    cfg_p: &PathBuf,
) -> Option<nwg::Bitmap> {
    if let Some(ico_p) = get_icon_p("", "", cfg_icon_s, cfg_p, &false) {
        let cfg_pkey_s = cfg_p.display().to_string();
        let mut cfg_icon_bitmap = Default::default();
        if let Ok(()) = nwg::Bitmap::builder()
            .source_file(Some(&ico_p))
            .strict(false)
            .size(Some((24, 24)))
            .build(&mut cfg_icon_bitmap)
        {
            debug!("✓ main 0 config: using icon for {}", cfg_pkey_s);
            menu_item.set_bitmap(Some(&cfg_icon_bitmap));
            return Some(cfg_icon_bitmap);
        } else {
            debug!(
                "✗ main 0 icon ✓ icon path, will be using DEFAULT icon for {:?}",
                cfg_p
            );
        }
    }
    menu_item.set_bitmap(None);
    None
}

impl SystemTray {
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
                if let Some(cfg_icon_bitmap) = img_dyn.get(cfg_p) {
                    self.tray_1cfg_m.set_bitmap(cfg_icon_bitmap.as_ref());
                }
            }
        } else {
            trace!("config menu item icon missing, read config and add it (or nothing) {cfg_p:?}");
            if let Ok(cfg) = cfg::new_from_file(cfg_p) {
                if let Some(cfg_icon_s) = cfg.options.tray_icon {
                    debug!("loaded config without a tray icon {cfg_p:?}");
                    if let Some(cfg_icon_bitmap) =
                        set_menu_item_cfg_icon(menu_item_cfg, &cfg_icon_s, cfg_p)
                    {
                        if is_active {
                            self.tray_1cfg_m.set_bitmap(Some(&cfg_icon_bitmap));
                        } // update currently active config's icon in the combo menu
                        debug!("✓set icon {cfg_p:?}");
                        let _ = img_dyn.insert(cfg_p.clone(), Some(cfg_icon_bitmap));
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
                debug!("✗ kanata config is locked, can't get current config (likely the gui changed the layer and is still holding the lock, it will update the icon)");
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
                let cfg_icon = &k.tray_icon;
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
            let cfg_icon = &k.tray_icon;
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

            {
                let mut app_data = self.app_data.borrow_mut();
                app_data.cfg_icon.clone_from(cfg_icon);
                app_data.layer0_name.clone_from(&k.layer_info[0].name);
                app_data.layer0_icon = Some(k.layer_info[0].name.clone());
                app_data.icon_match_layer_name = k.icon_match_layer_name;
                self.tray.set_tip(&cfg_layer_pkey_s); // update tooltip to point to the newer config
            }
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
        self.tray.show(
            &msg_content,
            Some(&msg_title),
            Some(flags),
            Some(&self.icon),
        );
        Ok(())
    }
    /// Update tray icon data on layer change
    fn reload_layer_icon(&self) {
        if let Some(cfg) = CFG.get() {
            if let Some(k) = cfg.try_lock() {
                let paths = &k.cfg_paths;
                let idx_cfg = k.cur_cfg_idx;
                let path_cur = &paths[idx_cfg];
                let path_cur_cc = path_cur.clone();
                let cfg_icon = &k.tray_icon;
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

                self.tray.set_tip(&cfg_layer_pkey_s); // update tooltip to point to the newer config
                let clear = false;
                self.update_tray_icon(
                    cfg_layer_pkey,
                    &cfg_layer_pkey_s,
                    layer_name,
                    layer_icon,
                    path_cur_cc,
                    clear,
                )
            } else {
                debug!("✗ kanata config is locked, can't get current layer (likely the gui changed the layer and is still holding the lock, it will update the icon)");
            }
        } else {
            warn!("✗ Layer indicator NOT changed, no CFG");
        };
    }
    /// Update tray icon data given various config/layer info
    fn update_tray_icon(
        &self,
        cfg_layer_pkey: PathBuf,
        cfg_layer_pkey_s: &str,
        layer_name: &str,
        layer_icon: &Option<String>,
        path_cur_cc: PathBuf,
        clear: bool,
    ) {
        let mut icon_dyn = self.icon_dyn.borrow_mut(); // update the tray icon
        let mut icon_active = self.icon_active.borrow_mut(); // update the tray icon active path
        let mut img_dyn = self.img_dyn.borrow_mut(); // update the tray images
        if clear {
            *icon_dyn = Default::default();
            *icon_active = Default::default();
            *img_dyn = Default::default();
            debug!("reloading active config, clearing icon_dyn/_active cache");
        }
        let app_data = self.app_data.borrow();
        if let Some(icon_opt) = icon_dyn.get(&cfg_layer_pkey) {
            // 1a config+layer path has already been checked
            if let Some(icon) = icon_opt {
                self.tray.set_icon(icon);
                *icon_active = Some(cfg_layer_pkey);
            } else {
                debug!(
                    "no icon found, using default for config+layer = {}",
                    cfg_layer_pkey_s
                );
                self.tray.set_icon(&self.icon);
                *icon_active = Some(cfg_layer_pkey);
            }
        } else if let Some(layer_icon) = layer_icon {
            // 1b cfg+layer path hasn't been checked, but layer has an icon configured, so check it
            if let Some(ico_p) = get_icon_p(
                layer_icon,
                layer_name,
                "",
                &path_cur_cc,
                &app_data.icon_match_layer_name,
            ) {
                let mut cfg_icon_bitmap = Default::default();
                if let Ok(()) = nwg::Bitmap::builder()
                    .source_file(Some(&ico_p))
                    .strict(false)
                    .build(&mut cfg_icon_bitmap)
                {
                    debug!(
                        "✓ Using an icon from this config+layer: {}",
                        cfg_layer_pkey_s
                    );
                    let temp_icon = cfg_icon_bitmap.copy_as_icon();
                    let _ = icon_dyn.insert(cfg_layer_pkey.clone(), Some(temp_icon));
                    *icon_active = Some(cfg_layer_pkey);
                    let temp_icon = cfg_icon_bitmap.copy_as_icon();
                    self.tray.set_icon(&temp_icon);
                } else {
                    warn!(
                        "✗ Invalid icon file \"{layer_icon}\" from this config+layer: {}",
                        cfg_layer_pkey_s
                    );
                    let _ = icon_dyn.insert(cfg_layer_pkey.clone(), None);
                    *icon_active = Some(cfg_layer_pkey);
                    self.tray.set_icon(&self.icon);
                }
            } else {
                warn!(
                    "✗ Invalid icon path \"{layer_icon}\" from this config+layer: {}",
                    cfg_layer_pkey_s
                );
                let _ = icon_dyn.insert(cfg_layer_pkey.clone(), None);
                *icon_active = Some(cfg_layer_pkey);
                self.tray.set_icon(&self.icon);
            }
        } else if icon_dyn.contains_key(&path_cur_cc) {
            // 2a no layer icon configured, but config icon exists, use it
            if let Some(icon) = icon_dyn.get(&path_cur_cc).unwrap() {
                self.tray.set_icon(icon);
                *icon_active = Some(path_cur_cc);
            } else {
                debug!(
                    "no icon found, using default for config: {}",
                    path_cur_cc.display().to_string()
                );
                self.tray.set_icon(&self.icon);
                *icon_active = Some(path_cur_cc);
            }
        } else {
            // 2a no layer icon configured, no config icon, use config path
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
                &app_data.icon_match_layer_name,
            ) {
                let mut cfg_icon_bitmap = Default::default();
                if let Ok(()) = nwg::Bitmap::builder()
                    .source_file(Some(&ico_p))
                    .strict(false)
                    .build(&mut cfg_icon_bitmap)
                {
                    debug!(
                        "✓ Using an icon from this config: {}",
                        path_cur_cc.display().to_string()
                    );
                    let temp_icon = cfg_icon_bitmap.copy_as_icon();
                    let _ = icon_dyn.insert(cfg_layer_pkey.clone(), Some(temp_icon));
                    *icon_active = Some(cfg_layer_pkey);
                    let temp_icon = cfg_icon_bitmap.copy_as_icon();
                    self.tray.set_icon(&temp_icon);
                } else {
                    warn!(
                        "✗ Invalid icon file \"{cfg_icon_p}\" from this config: {}",
                        path_cur_cc.display().to_string()
                    );
                    let _ = icon_dyn.insert(cfg_layer_pkey.clone(), None);
                    *icon_active = Some(cfg_layer_pkey);
                    self.tray.set_icon(&self.icon);
                }
            } else {
                warn!(
                    "✗ Invalid icon path \"{cfg_icon_p}\" from this config: {}",
                    path_cur_cc.display().to_string()
                );
                let _ = icon_dyn.insert(cfg_layer_pkey.clone(), None);
                *icon_active = Some(cfg_layer_pkey);
                self.tray.set_icon(&self.icon);
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
}

pub mod system_tray_ui {
    use super::*;
    use core::cmp;
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
            // Resources
            d.embed = Default::default();
            d.embed = nwg::EmbedResource::load(Some("kanata.exe"))?;
            nwg::Icon::builder()
                .source_embed(Some(&d.embed))
                .source_embed_str(Some("iconMain"))
                .strict(true) /*use sys, not panic, if missing*/
                .build(&mut d.icon)?;

            // Controls
            nwg::MessageWindow::builder().build(&mut d.window)?;
            nwg::Notice::builder()
                .parent(&d.window)
                .build(&mut d.layer_notice)?;
            nwg::Menu::builder()
                .parent(&d.window)
                .popup(true) /*context menu*/	//
                .build(&mut d.tray_menu)?;
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

            let mut tmp_bitmap = Default::default();
            nwg::Bitmap::builder()
                .source_embed(Some(&d.embed))
                .source_embed_str(Some("imgReload"))
                .strict(true)
                .size(Some((24, 24)))
                .build(&mut tmp_bitmap)?;
            let img_exit = nwg::Bitmap::from_system_icon(SIID_DELETE);
            d.tray_2reload.set_bitmap(Some(&tmp_bitmap));
            d.tray_3exit.set_bitmap(Some(&img_exit));
            d.img_reload = tmp_bitmap;
            d.img_exit = img_exit;

            let mut main_tray_icon_l = Default::default();
            let mut main_tray_icon_is = false;
            {
                let mut tray_item_dyn = d.tray_item_dyn.borrow_mut(); //extra scope to drop borrowed
                let mut icon_dyn = d.icon_dyn.borrow_mut();
                let mut img_dyn = d.img_dyn.borrow_mut();
                let mut icon_active = d.icon_active.borrow_mut();
                const MENU_ACC: &str = "ASDFGQWERTZXCVBYUIOPHJKLNM";
                let layer0_icon_s = &app_data.layer0_icon.clone().unwrap_or("".to_string());
                let cfg_icon_s = &app_data.cfg_icon.clone().unwrap_or("".to_string());
                if !(app_data.cfg_p).is_empty() {
                    for (i, cfg_p) in app_data.cfg_p.iter().enumerate() {
                        let i_acc = match i {
                            // accelerators from 1–0, A–Z starting from home row for easier presses
                            0..=8 => format!("&{} ", i + 1),
                            9 => format!("&{} ", 0),
                            10..=35 => format!(
                                "&{} ",
                                &MENU_ACC[(i - 10)..cmp::min(i - 10 + 1, MENU_ACC.len())]
                            ),
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
                                &app_data.icon_match_layer_name,
                            ) {
                                let mut cfg_layer_pkey = PathBuf::new(); // path key
                                cfg_layer_pkey.push(cfg_p.clone());
                                cfg_layer_pkey.push(PRE_LAYER.to_owned() + &app_data.layer0_name);
                                let cfg_layer_pkey_s = cfg_layer_pkey.display().to_string();
                                let mut cfg_icon_bitmap = Default::default();
                                if let Ok(()) = nwg::Bitmap::builder()
                                    .source_file(Some(&ico_p))
                                    .strict(false)
                                    .build(&mut cfg_icon_bitmap)
                                {
                                    debug!("✓ main 0 config: using icon for {}", cfg_layer_pkey_s);
                                    let temp_icon = cfg_icon_bitmap.copy_as_icon();
                                    let _ = icon_dyn.insert(cfg_layer_pkey, Some(temp_icon));
                                    let temp_icon = cfg_icon_bitmap.copy_as_icon();
                                    main_tray_icon_l = temp_icon;
                                    main_tray_icon_is = true;
                                } else {
                                    debug!("✗ main 0 icon ✓ icon path, will be using DEFAULT icon for {:?}",cfg_p);
                                    let _ = icon_dyn.insert(cfg_layer_pkey, None);
                                }
                            } else {
                                debug!("✗ main 0 config: using DEFAULT icon for {:?}", cfg_p);
                                let mut temp_icon = Default::default();
                                nwg::Icon::builder()
                                    .source_embed(Some(&d.embed))
                                    .source_embed_str(Some("iconMain"))
                                    .strict(true)
                                    .build(&mut temp_icon)?;
                                let _ = icon_dyn.insert(cfg_p.clone(), Some(temp_icon));
                                *icon_active = Some(cfg_p.clone());
                            }
                            // Set tray menu config item icons, ignores layers since these
                            // are per config
                            if let Some(cfg_icon_bitmap) =
                                set_menu_item_cfg_icon(&mut menu_item, cfg_icon_s, cfg_p)
                            {
                                d.tray_1cfg_m.set_bitmap(Some(&cfg_icon_bitmap)); // show currently
                                                                                  // active config's
                                                                                  // icon in the
                                                                                  // combo menu
                                let _ = img_dyn.insert(cfg_p.clone(), Some(cfg_icon_bitmap));
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
                                SystemTray::reload_layer_icon(&evt_ui);}
                        E::OnWindowClose =>
                            if handle == evt_ui.window {SystemTray::exit  (&evt_ui);}
                        E::OnMousePress(MousePressEvent::MousePressLeftUp) =>
                            if handle == evt_ui.tray {SystemTray::show_menu(&evt_ui);}
                        E::OnContextMenu/*🖰›*/ =>
                            if handle == evt_ui.tray {SystemTray::show_menu(&evt_ui);}
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
                                      if h_cfg_j.checked() {h_cfg_j.set_checked(false);} } // uncheck
                                      // others
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

pub fn build_tray(cfg: &Arc<Mutex<Kanata>>) -> Result<system_tray_ui::SystemTrayUi> {
    let k = cfg.lock();
    let paths = &k.cfg_paths;
    let cfg_icon = &k.tray_icon;
    let path_cur = &paths[0];
    let layer0_id = k.layout.b().current_layer();
    let layer0_name = &k.layer_info[layer0_id].name;
    let layer0_icon = &k.layer_info[layer0_id].icon;
    let icon_match_layer_name = &k.icon_match_layer_name;
    let app_data = SystemTrayData {
        tooltip: path_cur.display().to_string(),
        cfg_p: paths.clone(),
        cfg_icon: cfg_icon.clone(),
        layer0_name: layer0_name.clone(),
        layer0_icon: layer0_icon.clone(),
        icon_match_layer_name: *icon_match_layer_name,
    };
    let app = SystemTray {
        app_data: RefCell::new(app_data),
        ..Default::default()
    };
    Ok(SystemTray::build_ui(app)?)
}

pub use log::*;
pub use std::io::{stdout, IsTerminal};
pub use winapi::shared::minwindef::BOOL;
pub use winapi::um::wincon::{AttachConsole, FreeConsole, ATTACH_PARENT_PROCESS};

use once_cell::sync::Lazy;
pub static IS_TERM: Lazy<bool> = Lazy::new(|| stdout().is_terminal());
pub static IS_CONSOLE: Lazy<bool> =
    Lazy::new(|| unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0i32 });
