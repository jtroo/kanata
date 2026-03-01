use super::*;

use crate::anyhow_expr;
use crate::bail_expr;

pub(crate) fn parse_overrides(exprs: &[SExpr], s: &ParserState) -> Result<Overrides> {
    const ERR_MSG: &str =
        "defoverrides expects pairs of parameters: <input key list> <output key list>";
    let mut subexprs = check_first_expr(exprs.iter(), "defoverrides")?;

    let mut overrides = Vec::<Override>::new();
    while let Some(in_keys_expr) = subexprs.next() {
        let out_keys_expr = subexprs
            .next()
            .ok_or_else(|| anyhow_expr!(in_keys_expr, "Missing output keys for input keys"))?;
        let (in_keys, out_keys) = parse_override_inout_keys(in_keys_expr, out_keys_expr, s)?;
        overrides
            .push(Override::try_new(&in_keys, &out_keys).map_err(|e| anyhow!("{ERR_MSG}: {e}"))?);
    }
    log::debug!("All overrides:\n{overrides:#?}");
    Ok(Overrides::new(&overrides))
}

pub(crate) fn parse_override_inout_keys(
    in_keys_expr: &SExpr,
    out_keys_expr: &SExpr,
    s: &ParserState,
) -> Result<(Vec<OsCode>, Vec<OsCode>)> {
    let out_keys = out_keys_expr
        .list(s.vars())
        .ok_or_else(|| anyhow_expr!(out_keys_expr, "Output keys must be a list"))?;
    let in_keys = in_keys_expr
        .list(s.vars())
        .ok_or_else(|| anyhow_expr!(in_keys_expr, "Input keys must be a list"))?;
    let in_keys = in_keys
        .iter()
        .try_fold(vec![], |mut keys, key_expr| -> Result<Vec<OsCode>> {
            let key = key_expr
                .atom(s.vars())
                .and_then(str_to_oscode)
                .ok_or_else(|| {
                    anyhow_expr!(key_expr, "Unknown input key name, must use known keys")
                })?;
            keys.push(key);
            Ok(keys)
        })?;
    let out_keys =
        out_keys
            .iter()
            .try_fold(vec![], |mut keys, key_expr| -> Result<Vec<OsCode>> {
                let key = key_expr
                    .atom(s.vars())
                    .and_then(str_to_oscode)
                    .ok_or_else(|| {
                        anyhow_expr!(key_expr, "Unknown output key name, must use known keys")
                    })?;
                keys.push(key);
                Ok(keys)
            })?;
    Ok((in_keys, out_keys))
}

pub(crate) fn parse_overridesv2(exprs: &[SExpr], s: &ParserState) -> Result<Overrides> {
    const ERR_MSG: &str = "defoverridesv2 expects 4-tuples of parameters: <input key list> <output key list> <without mods> <excluded layers>";
    let mut subexprs = check_first_expr(exprs.iter(), "defoverridesv2")?;

    let mut overrides = Vec::<Override>::new();
    while let Some(in_keys_expr) = subexprs.next() {
        let out_keys_expr = subexprs
            .next()
            .ok_or_else(|| anyhow_expr!(in_keys_expr, "Missing output keys for input keys"))?;
        let (in_keys, out_keys) = parse_override_inout_keys(in_keys_expr, out_keys_expr, s)?;

        let without_mods_expr = subexprs
            .next()
            .ok_or_else(|| anyhow_expr!(in_keys_expr, "Missing without mods list"))?;
        let without_mods = without_mods_expr.list(s.vars()).ok_or_else(|| {
            anyhow_expr!(
                without_mods_expr,
                "Without mods configuration must be a list"
            )
        })?;

        let excluded_layers_expr = subexprs
            .next()
            .ok_or_else(|| anyhow_expr!(in_keys_expr, "Missing excluded layers"))?;
        let excluded_layers = excluded_layers_expr
            .list(s.vars())
            .ok_or_else(|| anyhow_expr!(excluded_layers_expr, "Excluded layers must be a list"))?;

        let without_mods =
            without_mods
                .iter()
                .try_fold(vec![], |mut keys, key_expr| -> Result<Vec<OsCode>> {
                    let key = key_expr
                        .atom(s.vars())
                        .and_then(str_to_oscode)
                        .ok_or_else(|| {
                            anyhow_expr!(key_expr, "Unknown key name, must use known keys")
                        })
                        .and_then(|osc| match osc.is_modifier() {
                            true => Ok(osc),
                            false => bail_expr!(
                                key_expr,
                                "Keys in without mods must be modifiers, e.g. lctl, ralt"
                            ),
                        })?;
                    keys.push(key);
                    Ok(keys)
                })?;

        let excluded_layers = excluded_layers.iter().try_fold(
            vec![],
            |mut layers, layer_expr| -> Result<Vec<u16>> {
                let layer = layer_expr
                    .atom(s.vars())
                    .and_then(|l| s.layer_idxs.get(l))
                    .ok_or_else(|| anyhow_expr!(layer_expr, "Unknown layer name"))?;
                layers.push(*layer as u16);
                Ok(layers)
            },
        )?;

        overrides.push(
            Override::try_new_v2(
                &in_keys,
                &out_keys,
                without_mods.into(),
                excluded_layers.into(),
            )
            .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?,
        );
    }
    log::debug!("All overrides:\n{overrides:#?}");
    Ok(Overrides::new(&overrides))
}
