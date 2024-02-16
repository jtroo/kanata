//! This file is responsible for template expansion.
//! For simplicity of implementation, there is performance left off the table.
//! This code runs at parse time and not in runtime
//! so it is not performance critical.
//!
//! The known performance left off the table is:
//!
//! - Creating the expanded template recurses through all SExprs every time.
//!   Instead the code could pre-compute the paths to access every variable
//!   that needs substition. (perf_1)
//!
//! - Replacing the `template-expand` items with the expanded template
//!   recreates the Vec for every replacement that happens at that layer.
//!   Instead the code could do a single pass
//!   and intelligently insert SExprs at the proper places. (perf_2)

use crate::anyhow_expr;
use crate::anyhow_span;
use crate::bail_expr;
use crate::bail_span;
use crate::err_span;

use super::error::*;
use super::sexpr::*;
use super::*;

#[derive(Debug)]
pub struct Template {
    name: String,
    vars: Vec<String>,
    // Same as vars above but all names are prefixed with '$'.
    vars_substitute_names: Vec<String>,
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
            .get(1)
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
            .get(2)
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
        let vars_substitute_names: Vec<_> = vars.iter().map(|v| format!("${v}")).collect();

        // Validate content of template
        let content: Vec<SExpr> = list.t.iter().skip(3).cloned().collect();
        let mut var_usage_counts: HashMap<String, u32> = vars_substitute_names
            .iter()
            .map(|v| (v.clone(), 0))
            .collect();
        visit_validate_all_atoms(&content, &mut |s| match s.t.as_str() {
            "deftemplate" => err_span!(s, "deftemplate is not allowed within deftemplate"),
            "template-expand" => err_span!(s, "template-expand is not allowed within deftemplate"),
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
            vars_substitute_names,
            content,
        });
    }

    // Find and do expansions
    let mut toplevels: Vec<SExpr> = toplevel_exprs
        .into_iter()
        .map(|tl| {
            SExpr::List(Spanned {
                span: tl.span,
                t: tl.t,
            })
        })
        .collect();
    expand(&mut toplevels, &templates)?;

    toplevels.into_iter().try_fold(vec![], |mut tls, tl| {
        tls.push(match &tl {
            SExpr::Atom(_) => bail_expr!(
                &tl,
                "expansion created a string outside any list which is not allowed"
            ),
            SExpr::List(l) => Spanned {
                t: l.t.clone(),
                span: l.span.clone(),
            },
        });
        Ok(tls)
    })
}

fn visit_validate_all_atoms(
    exprs: &[SExpr],
    visit: &mut dyn FnMut(&Spanned<String>) -> Result<()>,
) -> Result<()> {
    for expr in exprs {
        match expr {
            SExpr::Atom(a) => visit(a)?,
            SExpr::List(l) => visit_validate_all_atoms(&l.t, visit)?,
        }
    }
    Ok(())
}

fn visit_mut_all_atoms(exprs: &mut [SExpr], visit: &mut dyn FnMut(&mut SExpr)) {
    for expr in exprs {
        match expr {
            SExpr::Atom(_) => visit(expr),
            SExpr::List(l) => visit_mut_all_atoms(&mut l.t, visit),
        }
    }
}

struct Replacement {
    exprs: Vec<SExpr>,
    insert_index: usize,
}

fn expand(exprs: &mut Vec<SExpr>, templates: &[Template]) -> Result<()> {
    let mut replacements: Vec<Replacement> = vec![];
    for (expr_index, expr) in exprs.iter_mut().enumerate() {
        match expr {
            SExpr::Atom(_) => continue,
            SExpr::List(l) => {
                if !matches!(
                    l.t.first().and_then(|expr| expr.atom(None)),
                    Some("template-expand")
                ) {
                    expand(&mut l.t, templates)?;
                    continue;
                }

                // found expand, now parse
                let template =
                    l.t.get(1)
                        .ok_or_else(|| {
                            anyhow_span!(
                                l,
                                "template-expand must have a template name as the first parameter"
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

                let var_substitutions = l.t.iter().skip(2);
                let mut expanded_template = template.content.clone();
                // perf_1 : could store substitution knowledge instead of iterating and searching
                // every time
                visit_mut_all_atoms(&mut expanded_template, &mut |expr: &mut SExpr| {
                    *expr = match expr {
                        SExpr::Atom(a) => {
                            match template
                                .vars_substitute_names
                                .iter()
                                .enumerate()
                                .find(|(_, var)| *var == &a.t)
                            {
                                None => expr.clone(),
                                Some((var_index, _)) => var_substitutions
                                    .clone()
                                    .nth(var_index)
                                    .expect("validated matching var lens")
                                    .clone(),
                            }
                        }
                        // Below should not be reached because only atoms should be visited
                        SExpr::List(_) => unreachable!(),
                    }
                });

                replacements.push(Replacement {
                    insert_index: expr_index,
                    exprs: expanded_template,
                });
            }
        }
    }

    // Ensure replacements are sorted. They probably are, but may as well make sure.
    replacements.sort_by_key(|r| r.insert_index);
    // Must replace last-first to keep unreplaced insertion points stable.
    // perf_2 : could construct vec in one pass.
    for replacement in replacements.iter().rev() {
        let (before, after) = exprs.split_at(replacement.insert_index);
        let after = after.iter().skip(1); // after includes the variable to replace.
        let new_vec = before
            .iter()
            .cloned()
            .chain(replacement.exprs.iter().cloned())
            .chain(after.cloned())
            .collect();
        *exprs = new_vec;
    }

    Ok(())
}
