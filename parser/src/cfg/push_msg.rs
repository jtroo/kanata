use super::*;

use crate::bail;

pub(crate) fn parse_push_message(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!(
            "{PUSH_MESSAGE} expects at least one item, an item can be a list or an atom, found 0, none"
        );
    }
    let message = to_simple_expr(ac_params, s);
    custom(CustomAction::PushMessage(s.a.sref_vec(message)), &s.a)
}

pub(crate) fn to_simple_expr(params: &[SExpr], s: &ParserState) -> Vec<SimpleSExpr> {
    let mut result: Vec<SimpleSExpr> = Vec::new();
    for param in params {
        if let Some(a) = param.atom(s.vars()) {
            result.push(SimpleSExpr::Atom(a.trim_atom_quotes().to_owned()));
        } else {
            // unwrap: this must be a list, since it's not an atom.
            let sexps = param.list(s.vars()).unwrap();
            let value = to_simple_expr(sexps, s);
            let list = SimpleSExpr::List(value);
            result.push(list);
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleSExpr {
    Atom(String),
    List(Vec<SimpleSExpr>),
}
