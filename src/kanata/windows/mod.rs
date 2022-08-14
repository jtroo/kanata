use anyhow::{bail, Result};

use parking_lot::Mutex;

use crate::cfg;
use crate::kanata::*;

#[cfg(not(feature = "interception_driver"))]
mod llhook;
#[cfg(not(feature = "interception_driver"))]
pub use llhook::*;

#[cfg(feature = "interception_driver")]
mod interception;
#[cfg(feature = "interception_driver")]
pub use self::interception::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltGrBehaviour {
    DoNothing,
    CancelLctlPress,
    AddLctlRelease,
}

pub static ALTGR_BEHAVIOUR: Lazy<Mutex<AltGrBehaviour>> =
    Lazy::new(|| Mutex::new(AltGrBehaviour::DoNothing));

pub fn set_win_altgr_behaviour(cfg: &cfg::Cfg) -> Result<()> {
    *ALTGR_BEHAVIOUR.lock() = {
        const CANCEL: &str = "cancel-lctl-press";
        const ADD: &str = "add-lctl-release";
        match cfg.items.get("windows-altgr") {
            None => AltGrBehaviour::DoNothing,
            Some(cfg_val) => match cfg_val.as_str() {
                CANCEL => AltGrBehaviour::CancelLctlPress,
                ADD => AltGrBehaviour::AddLctlRelease,
                _ => bail!(
                    "Invalid value for windows-altgr: {}. Valid values are {},{}",
                    cfg_val,
                    CANCEL,
                    ADD
                ),
            },
        }
    };
    Ok(())
}
