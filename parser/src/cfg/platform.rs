use super::*;
use crate::anyhow_expr;
use crate::bail_span;
use crate::err_expr;

pub(crate) fn filter_platform_specific_cfg(
    top_levels: Vec<TopLevel>,
    deflocalkeys_variant_to_apply: &str,
    lsp_hint_inactive_code: &mut Vec<LspHintInactiveCode>,
) -> Result<Vec<TopLevel>> {
    let valid_platform_names = DEFLOCALKEYS_VARIANTS
        .iter()
        .map(|dfl| dfl.trim_start_matches("deflocalkeys-"))
        .collect::<Vec<_>>();
    let current_platform = deflocalkeys_variant_to_apply.trim_start_matches("deflocalkeys-");
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
                lsp_hint_inactive_code.push(LspHintInactiveCode {
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
