use super::*;

use crate::bail;
#[cfg(target_os = "macos")]
use crate::{anyhow_expr, bail_expr};

pub(crate) fn parse_set_input_source(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let id = parse_input_source_id(ac_params, SET_INPUT_SOURCE, s)?;
    custom(CustomAction::SetInputSource(id), &s.a)
}

pub(crate) fn parse_input_source_is(ac_params: &[SExpr], s: &ParserState) -> Result<KanataCustom> {
    let id = parse_input_source_id(ac_params, "input-source-is", s)?;
    Ok(s.a.sref(CustomAction::InputSourceIs(id)))
}

fn parse_input_source_id(
    ac_params: &[SExpr],
    action_name: &str,
    s: &ParserState,
) -> Result<&'static str> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ac_params, s);
        bail!("{action_name} is only supported on macOS");
    }

    #[cfg(target_os = "macos")]
    {
        if ac_params.len() != 1 {
            bail!(
                "{action_name} expects exactly one string input source ID, found {}",
                ac_params.len()
            );
        }
        let id = ac_params[0]
            .atom(s.vars())
            .ok_or_else(|| anyhow_expr!(&ac_params[0], "input source ID must be a string"))?
            .trim_atom_quotes();
        if id.is_empty() {
            bail_expr!(&ac_params[0], "input source ID must not be empty");
        }
        Ok(s.a.sref_str(id.to_string()))
    }
}
