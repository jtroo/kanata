use crate::anyhow_expr;
use crate::anyhow_span;
use crate::bail_expr;
use crate::bail_span;

use super::error::*;
use super::sexpr::*;
use super::*;

use itertools::Itertools;

#[derive(Debug)]
pub struct Template {
    name: String,
    vars: Vec<String>,
    content: Vec<SExpr>,
}

/// Parse and expand deftemplates.
///
/// Syntax of deftemplate is:
///
/// (deftemplate (<list of template vars>) <rest of template>)
pub fn expand_templates(mut toplevel_exprs: Vec<TopLevel>) -> Result<Vec<TopLevel>> {
    let mut templates: Vec<Template> = vec![];

    // Find defined templates
    for list in toplevel_exprs.iter_mut() {
        if !matches!(
            list.t.first().and_then(|expr| expr.atom(None)),
            Some("deftemplate")
        ) {
            continue;
        }

        // Parse template name
        let name = list
            .t
            .iter()
            .nth(1)
            .ok_or_else(|| {
                anyhow_span!(
                    list,
                    "deftemplate must have the template name as the first parameter"
                )
            })
            .and_then(|name_expr| {
                let name = name_expr
                    .atom(None)
                    .ok_or_else(|| anyhow_expr!(name_expr, "template name must be a string"))?;
                // check for duplicates
                if templates.iter().any(|t| t.name == name) {
                    bail_expr!(name_expr, "template name was already defined earlier");
                }
                Ok(name)
            })?
            .to_owned();

        // Parse template variable names
        let vars = list
            .t
            .iter()
            .nth(2)
            .ok_or_else(|| {
                anyhow_span!(list, "deftemplate must have a list as the second parameter")
            })
            .and_then(|v| {
                v.list(None).ok_or_else(|| {
                    anyhow_expr!(v, "deftemplate must have a list as the second parameter")
                })
            })
            .and_then(|v| {
                v.iter().try_fold(vec![], |mut vars, var| {
                    let s = var.atom(None).map(|a| a.to_owned()).ok_or_else(|| {
                        anyhow_expr!(var, "deftemplate variables must be strings")
                    })?;
                    vars.push(s);
                    Ok(vars)
                })
            })?;

        // Validate content of template
        let content: Vec<SExpr> = list.t.iter().skip(3).cloned().collect();
        let mut var_usage_counts: HashMap<String, u32> =
            vars.iter().map(|v| (v.clone(), 0)).collect();
        visit_validate_all_atoms(&content, |s| match s.t.as_str() {
            "deftemplate" => bail_span!(s, "deftemplate is not allowed within deftemplate"),
            "template-expand" => bail_span!(s, "template-expand is not allowed within deftemplate"),
            s => {
                if let Some(count) = var_usage_counts.get_mut(s) {
                    *count += 1;
                }
                Ok(())
            }
        })?;
        for (var, count) in var_usage_counts.iter() {
            if *count == 0 {
                log::warn!("deftemplate variable {var} did not appear in its template");
            }
        }

        templates.push(Template {
            name,
            vars,
            content,
        });
    }

    // Find and do expansions
    for list in toplevel_exprs.iter_mut() {
        expand(&mut list.t, &templates)?;
    }

    todo!()
}

fn visit_validate_all_atoms(
    exprs: &[SExpr],
    mut visit: impl FnMut(&Spanned<String>) -> Result<()>,
) -> Result<()> {
    for expr in exprs {
        match expr {
            SExpr::Atom(a) => visit(a)?,
            SExpr::List(l) => visit_validate_all_atoms(&l.t, &mut visit)?,
        }
    }
    Ok(())
}

fn expand(exprs: &mut Vec<SExpr>, templates: &[Template]) -> Result<()> {
    for (i, expr) in exprs.iter_mut().enumerate() {
        match expr {
            SExpr::Atom(_) => continue,
            SExpr::List(l) => {
                if !matches!(
                    l.t.first().and_then(|expr| expr.atom(None)),
                    Some("expand-template")
                ) {
                    expand(&mut l.t, templates)?;
                    continue;
                }

                // found expand, now parse
                let template =
                    l.t.iter()
                        .nth(1)
                        .ok_or_else(|| {
                            anyhow_span!(
                                l,
                                "expand-template must have a template name as the first parameter"
                            )
                        })
                        .and_then(|name_expr| {
                            let name = name_expr.atom(None).ok_or_else(|| {
                                anyhow_expr!(name_expr, "template name must be a string")
                            })?;
                            templates.iter().find(|t| t.name == name).ok_or_else(|| {
                                anyhow_expr!(
                                    name_expr,
                                    "template name was not defined in any deftemplate"
                                )
                            })
                        })?;
                if l.t.len() - 2 != template.vars.len() {
                    bail_span!(l, "template-expand of {} needs {} parameters but instead found {}.\nParameters: {}",
                    &template.name, template.vars.len(), l.t.len() - 2, template.vars.join(" "));
                }
                // generate exprs to replace/insert
                // save index for replace/insert later
            }
        }
    }
    // replace/insert later
    Ok(())
}
