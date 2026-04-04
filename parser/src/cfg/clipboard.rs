use super::*;

use crate::anyhow_expr;
use crate::bail;
use crate::bail_expr;

pub(crate) fn parse_clipboard_set(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 1 parameter: <clipboard string>";
    if ac_params.len() != 1 {
        bail!("{CLIPBOARD_SET} {ERR_MSG}, found {}", ac_params.len());
    }
    let expr = &ac_params[0];
    let clip_string = match expr {
        SExpr::Atom(filepath) => filepath,
        SExpr::List(_) => {
            bail_expr!(&expr, "Clipboard string cannot be a list")
        }
    };
    let clip_string = clip_string.t.trim_atom_quotes();
    custom(
        CustomAction::ClipboardSet(s.a.sref_str(clip_string.to_string())),
        &s.a,
    )
}

pub(crate) fn parse_clipboard_save(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 1 parameter: <clipboard save id (0-65535)>";
    if ac_params.len() != 1 {
        bail!("{CLIPBOARD_SAVE} {ERR_MSG}, found {}", ac_params.len());
    }
    let id = parse_u16(&ac_params[0], s, "clipboard save ID")?;
    custom(CustomAction::ClipboardSave(id), &s.a)
}

pub(crate) fn parse_clipboard_restore(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 1 parameter: <clipboard save id (0-65535)>";
    if ac_params.len() != 1 {
        bail!("{CLIPBOARD_RESTORE} {ERR_MSG}, found {}", ac_params.len());
    }
    let id = parse_u16(&ac_params[0], s, "clipboard save ID")?;
    custom(CustomAction::ClipboardRestore(id), &s.a)
}

pub(crate) fn parse_clipboard_save_swap(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str =
        "expects 2 parameters: <clipboard save id (0-65535)> <clipboard save id #2>";
    if ac_params.len() != 2 {
        bail!("{CLIPBOARD_SAVE_SWAP} {ERR_MSG}, found {}", ac_params.len());
    }
    let id1 = parse_u16(&ac_params[0], s, "clipboard save ID")?;
    let id2 = parse_u16(&ac_params[1], s, "clipboard save ID")?;
    custom(CustomAction::ClipboardSaveSwap(id1, id2), &s.a)
}

pub(crate) fn parse_clipboard_save_set(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 2 parameters: <clipboard save id (0-65535)> <save content>";
    if ac_params.len() != 2 {
        bail!("{CLIPBOARD_SAVE_SET} {ERR_MSG}, found {}", ac_params.len());
    }
    let id = parse_u16(&ac_params[0], s, "clipboard save ID")?;
    let save_content = ac_params[1]
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(&ac_params[1], "save content must be a string"))?;
    custom(
        CustomAction::ClipboardSaveSet(id, s.a.sref_str(save_content.into())),
        &s.a,
    )
}
