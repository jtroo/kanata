use crate::cfg::error::*;

use crate::cfg::custom;
use crate::cfg::KanataAction;
use crate::cfg::ParseError;
use crate::cfg::ParserState;
use crate::cfg::SExpr;
use crate::cfg::str_ext::TrimAtomQuotes;
use crate::custom_action::CustomAction;
use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WinMsgSid(String);
impl Default for WinMsgSid {
    fn default() -> Self {Self("kanata_4117d2917ccb4678a7a8c71a5ff898ed".to_string())} //TODO: replace with str
}
impl Into<String> for WinMsgSid {
    fn into(self) -> String {self.0}  }
impl Deref for WinMsgSid {type Target = String; fn deref(&self) -> &Self::Target {&self.0}}
impl<T> AsRef<T> for WinMsgSid
    where T: ?Sized, <WinMsgSid as Deref>::Target: AsRef<T>, {
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}
impl From<String> for WinMsgSid {
    fn from(s: String) -> Self {WinMsgSid(s            )}  }
impl From<&str> for WinMsgSid {
    fn from(s: &str  ) -> Self {WinMsgSid(s.to_string())}  }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WinMsgTarget {
    win_class: String,
    win_name : String,
}
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WinMsg {
    pub win_tgt : WinMsgTarget,
    pub msg_sid : WinMsgSid,
    pub argu    : usize,
    pub argi    : isize,
}
impl Default for WinMsgTarget {
    fn default() -> Self { Self {
        win_class : "AutoHotkey".to_string(), //TODO: replace with str
        win_name  : "\\AutoHotkey.ahk".to_string(), }    }
}

use colored::*;
use num_format::{Locale, ToFormattedString};
pub fn to_win_msg(ac_params: &[SExpr], s: &ParserState, cmd_name: &str) -> Result<WinMsg> {
    const ERR_MSG: &str = "expects at most 5 parameters: 2 message numeric arguments, target window title, shared message string id, target window class";
    let cmd_name = cmd_name.blue().bold();
    if ac_params.len() > 5 {bail!("{} {}",cmd_name,ERR_MSG);}
    let mut win_tgt:WinMsgTarget = Default::default();
    let mut msg_sid = Default::default();
    let mut argu    = Default::default();
    let mut argi    = Default::default();
    if ac_params.len() > 0 { let arg = &ac_params[0];
        if let Some(a) = arg.atom(s.vars()) {
            argu = match str::parse::<usize>(a.trim_atom_quotes()) {
                Ok(argu) => argu,
                Err(_) => bail_expr!(&arg, "invalid numeric argument, expected {}–{}. {} {}",
                usize::MIN.to_formatted_string(&Locale::en).blue(),usize::MAX.to_formatted_string(&Locale::en).blue(),cmd_name,ERR_MSG),
            }
        } else {bail!("{ERR_MSG}");}
    }
    if ac_params.len() > 1 { let arg = &ac_params[1];
        if let Some(a) = arg.atom(s.vars()) {
            argi = match str::parse::<isize>(a.trim_atom_quotes()) {
                Ok(argi) => argi,
                Err(_) => bail_expr!(&arg, "invalid numeric argument, expected {}–{}. {} {}",
                isize::MIN.to_formatted_string(&Locale::en).blue(),isize::MAX.to_formatted_string(&Locale::en).blue(),cmd_name,ERR_MSG),
            }
        } else {bail!("{ERR_MSG}");}
    }
    if ac_params.len() > 2 { let arg = &ac_params[2];
        if let Some(a) = arg.atom(s.vars()) {let a = a.trim_atom_quotes();
            if ! a.is_empty() {win_tgt.win_name = a.to_string();}
        } else {bail_expr!(&arg, "invalid target window {}. {} {}","file name".blue(),cmd_name,ERR_MSG)}
    }
    if ac_params.len() > 3 { let arg = &ac_params[3];
        if let Some(a) = arg.atom(s.vars()) {let a = a.trim_atom_quotes();
            if ! a.is_empty() {msg_sid = a.into();}
        } else {bail_expr!(&arg, "invalid message shared {}. {} {}","string id".blue(),cmd_name,ERR_MSG)}
    }
    if ac_params.len() > 4 { let arg = &ac_params[4];
        if let Some(a) = arg.atom(s.vars()) {let a = a.trim_atom_quotes();
            if ! a.is_empty() {win_tgt.win_class = a.to_string();}
        } else {bail_expr!(&arg, "invalid target window {}. {} {}","class".blue(),cmd_name,ERR_MSG)}
    }
    Ok(WinMsg{win_tgt,msg_sid,argu,argi})
}

pub fn win_send_message(ac_params: &[SExpr], s: &ParserState, cmd_name: &str) -> Result<&'static KanataAction> {
    let win_msg = to_win_msg(ac_params, s, cmd_name)?;
    log::trace!("win_msg = {:?}",win_msg);
    custom(CustomAction::WinSendMessage(win_msg), &s.a)
}
pub fn win_post_message(ac_params: &[SExpr], s: &ParserState, cmd_name: &str) -> Result<&'static KanataAction> {
    let win_msg = to_win_msg(ac_params, s, cmd_name)?;
    log::trace!("win_msg = {:?}",win_msg);
    custom(CustomAction::WinPostMessage(win_msg), &s.a)
}
