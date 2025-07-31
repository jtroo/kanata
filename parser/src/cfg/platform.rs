use super::*;

use crate::anyhow_expr;
use crate::bail_expr;
use crate::bail_span;
use crate::err_expr;

pub(crate) fn filter_platform_specific_cfg(
    top_levels: Vec<TopLevel>,
    deflocalkeys_variant_to_apply: &str,
    _lsp_hints: &mut lsp_hints::LspHints,
) -> Result<Vec<TopLevel>> {
    let valid_platform_names = DEFLOCALKEYS_VARIANTS
        .iter()
        .map(|dfl| dfl.trim_start_matches("deflocalkeys-"))
        .collect::<Vec<_>>();
    let current_platform =
        deflocalkeys_variant_to_apply.trim_start_matches("deflocalkeys-");
    top_levels
        .into_iter()
        .try_fold(vec![], |mut tles, tle| -> Result<Vec<TopLevel>> {
            if !matches!(tle.t.first().and_then(|m| m.atom(None)), Some("platform")) {
                tles.push(tle);
                return Ok(tles);
            }

            if tle.t.len() != 3 {
                bail_span!(
                    &tle,
                    "platform requires exactly two parameters:\n\
                      applicable-platforms, configuration-item"
                );
            }

            let configuration = tle.t[2]
                .span_list(None)
                .ok_or_else(|| anyhow_expr!(&tle.t[2], "configuration-item must be a list"))?;

            let applicable_platforms = tle.t[1]
                .list(None)
                .ok_or_else(|| anyhow_expr!(&tle.t[1], "applicable-platforms must be a list"))
                .and_then(|pf_list| {
                    pf_list.iter().try_fold(vec![], |mut pfs, pf_expr| {
                        let good_pf = pf_expr
                            .atom(None)
                            .ok_or_else(|| anyhow_expr!(pf_expr, "platform must be a string"))
                            .and_then(|pf| {
                                if valid_platform_names.contains(&pf) {
                                    Ok(pf)
                                } else {
                                    err_expr!(
                                        pf_expr,
                                        "Unknown platform. Valid platforms:\n{}",
                                        valid_platform_names.join(" ")
                                    )
                                }
                            })?;
                        pfs.push(good_pf);
                        Ok(pfs)
                    })
                })?;

            if applicable_platforms.contains(&current_platform) {
                tles.push(configuration.clone());
            } else {
                #[cfg(feature = "lsp")]
                _lsp_hints.inactive_code.push(lsp_hints::InactiveCode {
                    span: tle.span.clone(),
                    reason: format!(
                        "Current platform \"{current_platform}\" doesn't match any of: {}",
                        applicable_platforms.join(" ")
                    ),
                })
            }

            Ok(tles)
        })
}

pub(crate) fn filter_env_specific_cfg(
    top_levels: Vec<TopLevel>,
    env: &EnvVars,
    _lsp_hints: &mut lsp_hints::LspHints,
) -> Result<Vec<TopLevel>> {
    top_levels
        .into_iter()
        .try_fold(vec![], |mut tles, tle| -> Result<Vec<TopLevel>> {
            if !matches!(
                tle.t.first().and_then(|m| m.atom(None)),
                Some("environment")
            ) {
                tles.push(tle);
                return Ok(tles);
            }
            let env = match env.as_ref() {
                Ok(v) => v,
                Err(e) => Err(anyhow!("{e}"))?,
            };
            if tle.t.len() != 3 {
                bail_span!(
                    &tle,
                    "environment requires exactly two parameters:\n\
                     varname-varvalue, configuration-item"
                );
            }

            let configuration = tle.t[2]
                .span_list(None)
                .ok_or_else(|| anyhow_expr!(&tle.t[2], "configuration-item must be a list"))?;

            let (env_var_name, env_var_val) = tle.t[1]
                .list(None)
                .ok_or_else(|| anyhow_expr!(&tle.t[1], "varname-varvalue must be a list"))
                .and_then(|varnameval| {
                    if varnameval.len() != 2 {
                        bail_expr!(
                            &tle.t[1],
                            "varname-varvalue must be a list of two elements:\n\
                                               varname, varvalue"
                        );
                    }
                    Ok((
                        varnameval[0].atom(None).ok_or_else(|| {
                            anyhow_expr!(&varnameval[0], "varname must be a string")
                        })?,
                        varnameval[1].atom(None).ok_or_else(|| {
                            anyhow_expr!(&varnameval[1], "varvalue must be a string")
                        })?,
                    ))
                })?;
            let env_var_val = env_var_val.trim_atom_quotes();
            match (
                env.iter().find_map(|(name, val)| {
                    if name == env_var_name {
                        Some(val)
                    } else {
                        None
                    }
                }),
                env_var_val.is_empty(),
            ) {
                (None, false) => {
                    #[cfg(feature = "lsp")]
                    _lsp_hints.inactive_code.push(lsp_hints::InactiveCode {
                        span: tle.span.clone(),
                        reason: format!(
                            "Active if env var {env_var_name} is {env_var_val}. It is unset."
                        ),
                    });
                }
                (None, true) => {
                    tles.push(configuration.clone());
                }
                (Some(val), true) if val.is_empty() => {
                    tles.push(configuration.clone());
                }
                (Some(_val), true) => {
                    #[cfg(feature = "lsp")]
                    _lsp_hints.inactive_code.push(lsp_hints::InactiveCode {
                        span: tle.span.clone(),
                        reason: format!(
                            "Active if {env_var_name} is empty or unset. It has value {_val}"
                        ),
                    });
                }
                (Some(val), false) => {
                    if val == env_var_val {
                        tles.push(configuration.clone());
                    } else {
                        #[cfg(feature = "lsp")]
                        _lsp_hints.inactive_code.push(lsp_hints::InactiveCode {
                            span: tle.span.clone(),
                            reason: format!(
                                "Active if {env_var_name} is {env_var_val}. It has value {val}"
                            ),
                        })
                    }
                }
            }

            Ok(tles)
        })
}
