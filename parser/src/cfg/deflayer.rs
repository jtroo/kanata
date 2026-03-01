use super::*;

use crate::anyhow_expr;
use crate::anyhow_span;
use crate::bail;
use crate::bail_expr;
use crate::bail_span;

pub(crate) type LayerIndexes = HashMap<String, usize>;

pub(crate) const DEFLAYER: &str = "deflayer";
pub(crate) const DEFLAYER_MAPPED: &str = "deflayermap";

/// Returns layer names and their indexes into the keyberon layout. This also checks that:
/// - All layers have the same number of items as the defsrc,
/// - There are no duplicate layer names
/// - Parentheses weren't used directly or kmonad-style escapes for parentheses weren't used.
pub(crate) fn parse_layer_indexes(
    exprs: &[SpannedLayerExprs],
    expected_len: usize,
    vars: &HashMap<String, SExpr>,
    _lsp_hints: &mut LspHints,
) -> Result<(LayerIndexes, LayerIcons)> {
    let mut layer_indexes = HashMap::default();
    let mut layer_icons = HashMap::default();
    for (i, expr_type) in exprs.iter().enumerate() {
        let (mut subexprs, expr, do_element_count_check, deflayer_keyword) = match expr_type {
            SpannedLayerExprs::DefsrcMapping(e) => {
                (check_first_expr(e.t.iter(), DEFLAYER)?, e, true, DEFLAYER)
            }
            SpannedLayerExprs::CustomMapping(e) => (
                check_first_expr(e.t.iter(), DEFLAYER_MAPPED)?,
                e,
                false,
                DEFLAYER_MAPPED,
            ),
        };
        let layer_expr = subexprs.next().ok_or_else(|| {
            anyhow_span!(
                expr,
                "{deflayer_keyword} requires a layer name after `{deflayer_keyword}` token"
            )
        })?;
        let (layer_name, _layer_name_span, icon) = {
            let name = layer_expr.atom(Some(vars));
            match name {
                Some(name) => (name.to_owned(), layer_expr.span(), None),
                None => {
                    // unwrap: this **must** be a list due to atom() call above.
                    let list = layer_expr.list(Some(vars)).unwrap();
                    let first = list.first().ok_or_else(|| anyhow_expr!(
                            layer_expr,
                            "{deflayer_keyword} requires a string name within this pair of parentheses (or a string name without any)"
                        ))?;
                    let name = first.atom(Some(vars)).ok_or_else(|| anyhow_expr!(
                            layer_expr,
                            "layer name after {deflayer_keyword} must be a string when enclosed within one pair of parentheses"
                        ))?;
                    let layer_opts = parse_layer_opts(&list[1..])?;
                    let icon = layer_opts
                        .get(DEFLAYER_ICON[0])
                        .map(|icon_s| icon_s.trim_atom_quotes().to_owned());
                    (name.to_owned(), first.span(), icon)
                }
            }
        };
        if layer_indexes.contains_key(&layer_name) {
            bail_expr!(layer_expr, "duplicate layer name: {}", layer_name);
        }
        // Check if user tried to use parentheses directly - `(` and `)`
        // or escaped them like in kmonad - `\(` and `\)`.
        for subexpr in subexprs {
            if let Some(list) = subexpr.list(None) {
                if list.is_empty() {
                    bail_expr!(
                        subexpr,
                        "You can't put parentheses in deflayer directly, because they are special characters for delimiting lists.\n\
                         To get `(` and `)` in US layout, you should use `S-9` and `S-0` respectively.\n\
                         For more context, see: https://github.com/jtroo/kanata/issues/459"
                    )
                }
                if list.len() == 1
                    && list
                        .first()
                        .is_some_and(|s| s.atom(None).is_some_and(|atom| atom == "\\"))
                {
                    bail_expr!(
                        subexpr,
                        "Escaping shifted characters with `\\` is currently not supported in kanata.\n\
                         To get `(` and `)` in US layout, you should use `S-9` and `S-0` respectively.\n\
                         For more context, see: https://github.com/jtroo/kanata/issues/163"
                    )
                }
            }
        }
        if do_element_count_check {
            let num_actions = expr.t.len() - 2;
            if num_actions != expected_len {
                bail_span!(
                    expr,
                    "Layer {} has {} item(s), but requires {} to match defsrc",
                    layer_name,
                    num_actions,
                    expected_len
                )
            }
        }

        #[cfg(feature = "lsp")]
        _lsp_hints
            .definition_locations
            .layer
            .insert(layer_name.clone(), _layer_name_span.clone());

        layer_indexes.insert(layer_name.clone(), i);
        layer_icons.insert(layer_name, icon);
    }

    Ok((layer_indexes, layer_icons))
}

pub(crate) fn parse_layers(
    s: &ParserState,
    mapped_keys: &mut MappedKeys,
    defcfg: &CfgOptions,
) -> Result<IntermediateLayers> {
    let mut layers_cfg = new_layers(s.layer_exprs.len());
    if s.layer_exprs.len() > MAX_LAYERS {
        bail!("Maximum number of layers ({}) exceeded.", MAX_LAYERS);
    }
    let mut defsrc_layer = s.defsrc_layer;
    for (layer_level, layer) in s.layer_exprs.iter().enumerate() {
        match layer {
            // The skip is done to skip the `deflayer` and layer name tokens.
            LayerExprs::DefsrcMapping(layer) => {
                // Parse actions in the layer and place them appropriately according
                // to defsrc mapping order.
                for (i, ac) in layer.iter().skip(2).enumerate() {
                    let ac = parse_action(ac, s)?;
                    layers_cfg[layer_level][0][s.mapping_order[i]] = *ac;
                }
            }
            LayerExprs::CustomMapping(layer) => {
                // Parse actions as input output pairs
                let mut pairs = layer[2..].chunks_exact(2);
                let mut layer_mapped_keys = HashSet::default();
                let mut defsrc_anykey_used = false;
                let mut unmapped_anykey_used = false;
                let mut both_anykey_used = false;
                for pair in pairs.by_ref() {
                    let input = &pair[0];
                    let action = &pair[1];

                    let action = parse_action(action, s)?;
                    if input.atom(s.vars()).is_some_and(|x| x == "_") {
                        if defsrc_anykey_used {
                            bail_expr!(input, "must have only one use of _ within a layer")
                        }
                        if both_anykey_used {
                            bail_expr!(input, "must either use _ or ___ within a layer, not both")
                        }
                        for i in 0..s.mapping_order.len() {
                            if layers_cfg[layer_level][0][s.mapping_order[i]] == DEFAULT_ACTION {
                                layers_cfg[layer_level][0][s.mapping_order[i]] = *action;
                            }
                        }
                        defsrc_anykey_used = true;
                    } else if input.atom(s.vars()).is_some_and(|x| x == "__") {
                        if unmapped_anykey_used {
                            bail_expr!(input, "must have only one use of __ within a layer")
                        }
                        if !defcfg.process_unmapped_keys {
                            bail_expr!(
                                input,
                                "must set process-unmapped-keys to yes to use __ to map unmapped keys"
                            );
                        }
                        if both_anykey_used {
                            bail_expr!(input, "must either use __ or ___ within a layer, not both")
                        }
                        for i in 0..layers_cfg[0][0].len() {
                            if layers_cfg[layer_level][0][i] == DEFAULT_ACTION
                                && !s.mapping_order.contains(&i)
                            {
                                layers_cfg[layer_level][0][i] = *action;
                            }
                        }
                        unmapped_anykey_used = true;
                    } else if input.atom(s.vars()).is_some_and(|x| x == "___") {
                        if both_anykey_used {
                            bail_expr!(input, "must have only one use of ___ within a layer")
                        }
                        if defsrc_anykey_used {
                            bail_expr!(input, "must either use _ or ___ within a layer, not both")
                        }
                        if unmapped_anykey_used {
                            bail_expr!(input, "must either use __ or ___ within a layer, not both")
                        }
                        if !defcfg.process_unmapped_keys {
                            bail_expr!(
                                input,
                                "must set process-unmapped-keys to yes to use ___ to also map unmapped keys"
                            );
                        }
                        for i in 0..layers_cfg[0][0].len() {
                            if layers_cfg[layer_level][0][i] == DEFAULT_ACTION {
                                layers_cfg[layer_level][0][i] = *action;
                            }
                        }
                        both_anykey_used = true;
                    } else {
                        let input_key = input
                            .atom(s.vars())
                            .and_then(str_to_oscode)
                            .ok_or_else(|| anyhow_expr!(input, "input must be a key name"))?;
                        mapped_keys.insert(input_key);
                        if !layer_mapped_keys.insert(input_key) {
                            bail_expr!(input, "input key must not be repeated within a layer")
                        }
                        layers_cfg[layer_level][0][usize::from(input_key)] = *action;
                    }
                }
                let rem = pairs.remainder();
                if !rem.is_empty() {
                    bail_expr!(&rem[0], "input must by followed by an action");
                }
            }
        }
        for (osc, layer_action) in layers_cfg[layer_level][0].iter_mut().enumerate() {
            if *layer_action == DEFAULT_ACTION {
                *layer_action = match s.block_unmapped_keys && !is_a_button(osc as u16) {
                    true => Action::NoOp,
                    false => Action::Trans,
                };
            }
        }

        // Set fake keys on every layer.
        for (y, action) in s.virtual_keys.values() {
            let (x, y) = get_fake_key_coords(*y);
            layers_cfg[layer_level][x as usize][y as usize] = **action;
        }

        // If the user has configured delegation to the first (default) layer for transparent keys,
        // (as opposed to delegation to defsrc), replace the defsrc actions with the actions from
        // the first layer.
        if layer_level == 0 && s.delegate_to_first_layer {
            for (defsrc_ac, default_layer_ac) in defsrc_layer.iter_mut().zip(layers_cfg[0][0]) {
                if default_layer_ac != Action::Trans {
                    *defsrc_ac = default_layer_ac;
                }
            }
        }

        // Very last thing - ensure index 0 is always no-op. This shouldn't have any way to be
        // physically activated. This enable other code to rely on there always being a no-op key.
        layers_cfg[layer_level][0][0] = Action::NoOp;
    }
    Ok(layers_cfg)
}

pub(crate) fn parse_layer_base(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let idx = layer_idx(ac_params, &s.layer_idxs, s)?;
    set_layer_change_lsp_hint(&ac_params[0], &mut s.lsp_hints.borrow_mut());
    Ok(s.a.sref(Action::DefaultLayer(idx)))
}

pub(crate) fn parse_layer_toggle(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let idx = layer_idx(ac_params, &s.layer_idxs, s)?;
    set_layer_change_lsp_hint(&ac_params[0], &mut s.lsp_hints.borrow_mut());
    Ok(s.a.sref(Action::Layer(idx)))
}
