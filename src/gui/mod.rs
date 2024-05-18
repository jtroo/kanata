pub mod win;
pub use win::*;
pub mod win_nwg_ext;
pub mod win_dbg_logger;
pub use win_dbg_logger as log_win;
pub use win_dbg_logger::WINDBG_LOGGER;
pub use win_nwg_ext::*;

use crate::*;
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};
use std::sync::mpsc::{Sender as ASender};
pub static CFG: OnceLock<Arc<Mutex<Kanata>>> = OnceLock::new();
pub static GUI_TX: OnceLock<native_windows_gui::NoticeSender> = OnceLock::new();
pub static GUI_CFG_TX: OnceLock<native_windows_gui::NoticeSender> = OnceLock::new();
pub static GUI_ERR_TX: OnceLock<native_windows_gui::NoticeSender> = OnceLock::new();
pub static GUI_ERR_MSG_TX: OnceLock<ASender<(String,String)>> = OnceLock::new();
