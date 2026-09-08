use super::*;

use crate::bail;
use anyhow::anyhow;

pub(crate) fn parse_arbitrary_code(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "arbitrary code expects one parameter: <code: 0-767>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}");
    }
    let code = ac_params[0]
        .atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|c| c.ok())
        // The range in the message was never enforced. It has to be now: this
        // writes the code straight to the OS, and everything at or above
        // `PAD_SOUTH` begins the synthetic controller range, which has no
        // scancodes behind it.
        .filter(|code| *code < OsCode::PAD_SOUTH as u16)
        .ok_or_else(|| anyhow!("{ERR_MSG}: got {:?}", ac_params[0]))?;
    custom(CustomAction::SendArbitraryCode(code), &s.a)
}
