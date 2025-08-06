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
//! - Replacing the `template-expand|if-equal` items with the appropriate values
//!   recreates the Vec for every replacement that happens at that recursion depth.
//!   Instead the code could do recreate the vec only once
//!   and insert SExprs at the proper places. (perf_2)

use crate::anyhow_expr;
use crate::anyhow_span;
use crate::bail_expr;
use crate::bail_span;
use crate::err_expr;
use crate::err_span;

use super::error::*;
use super::sexpr::*;
use super::*;

#[derive(Debug)]
struct Template {
    name: String,
    vars: Vec<String>,
    // Same as vars above but all names are prefixed with '$'.
    vars_substitute_names: Vec<String>,
    content: Vec<SExpr>,
}

/// Parse `deftemplate`s and expand `template-expand`s.
///
/// Syntax of `deftemplate` is:
///
/// `(deftemplate <template name> (<list of template vars>) <rest of template>)`
///
/// Syntax of `template-expand` is:
///
/// `(template-expand <template name> <template var substitutions>)`
pub fn expand_templates(
    mut toplevel_exprs: Vec<TopLevel>,
    lsp_hints: &mut LspHints,
) -> Result<Vec<TopLevel>> {
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
        let (name, _name_span) = list
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
                Ok((name, name_expr.span()))
            })?;

        #[cfg(feature = "lsp")]
        lsp_hints
            .definition_locations
            .template
            .insert(name.to_owned(), _name_span);

        // Parse template variable names
        let vars = list
            .t
            .get(2)
            .ok_or_else(|| {
                anyhow_span!(
                    list,
                    "deftemplate must have a list of template variables as the second parameter"
                )
            })
            .and_then(|v| {
                v.list(None).ok_or_else(|| {
                    anyhow_expr!(
                        v,
                        "deftemplate must have a list of template variables the second parameter"
                    )
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
        visit_validate_all_atoms_peek_next(&content, &mut |s, s_next| match s.t.as_str() {
            "deftemplate" => err_span!(s, "deftemplate is not allowed within deftemplate"),
            "template-expand" | "t!" => {
                match s_next {
                    Some(next) => {
                        match next.atom(None) {
                            Some(name_in_expand) => {
                                match templates.iter().any(|existing_template| {
                                    existing_template.name == name_in_expand
                                }) {
                                    true => Ok(()),
                                    false => err_expr!(
                                        next,
                                        "Unknown template name in template-expand. Note that order of declaration matters."
                                    ),
                                }
                            }
                            None => {
                                // Next expr is list.
                                // This is invalid syntax, but this will be caught later.
                                // For simplicity in this function, leave it be.
                                Ok(())
                            }
                        }
                    }
                    None => {
                        // No next expr after expand.
                        // This is invalid syntax, but this will be caught later.
                        // For simplicity in this function, leave it be.
                        Ok(())
                    }
                }
            }
            s => {
                if let Some(count) = var_usage_counts.get_mut(s) {
                    *count += 1;
                }
                Ok(())
            }
        })?;
        for (var, count) in var_usage_counts.iter() {
            if *count == 0 {
                log::warn!("deftemplate variable {var} did not appear in its template {name}");
            }
        }

        templates.push(Template {
            name: name.to_string(),
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
    expand(&mut toplevels, &templates, lsp_hints)?;

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

struct Replacement {
    exprs: Vec<SExpr>,
    insert_index: usize,
}

fn expand(exprs: &mut Vec<SExpr>, templates: &[Template], _lsp_hints: &mut LspHints) -> Result<()> {
    let mut replacements: Vec<Replacement> = vec![];
    loop {
        for (expr_index, expr) in exprs.iter_mut().enumerate() {
            match expr {
                SExpr::Atom(_) => continue,
                SExpr::List(l) => {
                    if !matches!(
                        l.t.first().and_then(|expr| expr.atom(None)),
                        Some("template-expand") | Some("t!")
                    ) {
                        expand(&mut l.t, templates, _lsp_hints)?;
                        continue;
                    }

                    // found expand, now parse
                    let template = l
                        .t
                        .get(1)
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
                            #[cfg(feature = "lsp")]
                            _lsp_hints
                                .reference_locations
                                .template
                                .push(name, name_expr.span());
                            templates.iter().find(|t| t.name == name).ok_or_else(|| {
                                anyhow_expr!(
                                    name_expr,
                                    "template name was not defined in any deftemplate"
                                )
                            })
                        })?;
                    if l.t.len() - 2 != template.vars.len() {
                        bail_span!(
                            l,
                            "template-expand of {} needs {} parameters but instead found {}.\nParameters: {}",
                            &template.name,
                            template.vars.len(),
                            l.t.len() - 2,
                            template.vars.join(" ")
                        );
                    }

                    let var_substitutions = l.t.iter().skip(2);
                    let mut expanded_template = template.content.clone();
                    // Substitute variables.
                    // perf_1 : could store substitution knowledge instead of iterating and searching
                    // every time
                    visit_mut_all_atoms(&mut expanded_template, &mut |expr: &mut SExpr| {
                        *expr = match expr {
                            // Below should not be reached because only atoms should be visited
                            SExpr::List(_) => unreachable!(),
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
                        }
                    });

                    visit_mut_all_lists(&mut expanded_template, &mut |expr: &mut SExpr| {
                        *expr = match expr {
                            // Below should not be reached because only lists should be visited
                            SExpr::Atom(_) => unreachable!(),
                            SExpr::List(l) => parse_list_var(l, &HashMap::default()),
                        };
                        match expr {
                            SExpr::Atom(_) => true,
                            SExpr::List(_) => false,
                        }
                    });

                    while evaluate_conditionals(&mut expanded_template)? {}

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
            let after = after.iter().skip(1); // first element is `(template-expand ...)`
            let new_vec = before
                .iter()
                .cloned()
                .chain(replacement.exprs.iter().cloned())
                .chain(after.cloned())
                .collect();
            *exprs = new_vec;
        }

        if replacements.is_empty() {
            break;
        }
        replacements.clear();
    }

    Ok(())
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

#[allow(clippy::type_complexity)]
fn visit_validate_all_atoms_peek_next(
    exprs: &[SExpr],
    visit: &mut dyn FnMut(&Spanned<String>, Option<&SExpr>) -> Result<()>,
) -> Result<()> {
    for (i, expr) in exprs.iter().enumerate() {
        match expr {
            SExpr::Atom(a) => visit(a, exprs.get(i + 1))?,
            SExpr::List(l) => visit_validate_all_atoms_peek_next(&l.t, visit)?,
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

fn visit_mut_all_lists(exprs: &mut [SExpr], visit: &mut dyn FnMut(&mut SExpr) -> ChangeOccurred) {
    for expr in exprs {
        loop {
            if let SExpr::Atom(_) = expr {
                break;
            }
            // revisit until change did not happen to the list
            if !visit(expr) {
                if let SExpr::List(l) = expr {
                    visit_mut_all_lists(&mut l.t, visit);
                }
                break;
            }
        }
    }
}

type ChangeOccurred = bool;

fn evaluate_conditionals(exprs: &mut Vec<SExpr>) -> Result<ChangeOccurred> {
    let mut replacements: Vec<Replacement> = vec![];
    let mut expand_happened = false;
    for (index, expr) in exprs.iter_mut().enumerate() {
        if matches!(expr, SExpr::Atom(_)) {
            continue;
        }
        // expr must be a list, visit it
        if let Some(exprs) = if_equal_replacement(expr)? {
            replacements.push(Replacement {
                exprs,
                insert_index: index,
            })
        } else if let Some(exprs) = if_not_equal_replacement(expr)? {
            replacements.push(Replacement {
                exprs,
                insert_index: index,
            })
        } else if let Some(exprs) = if_in_list_replacement(expr)? {
            replacements.push(Replacement {
                exprs,
                insert_index: index,
            })
        } else if let Some(exprs) = if_not_in_list_replacement(expr)? {
            replacements.push(Replacement {
                exprs,
                insert_index: index,
            })
        } else {
            expand_happened |= match expr {
                SExpr::Atom(_) => unreachable!(),
                SExpr::List(l) => evaluate_conditionals(&mut l.t)?,
            };
        }
    }
    // Ensure replacements are sorted. They probably are, but may as well make sure.
    replacements.sort_by_key(|r| r.insert_index);
    // Must replace last-first to keep unreplaced insertion points stable.
    // perf_2 : could construct vec in one pass.
    for replacement in replacements.iter().rev() {
        let (before, after) = exprs.split_at(replacement.insert_index);
        let after = after.iter().skip(1); // first element is `(if-equal ...)`
        let new_vec = before
            .iter()
            .cloned()
            .chain(replacement.exprs.iter().cloned())
            .chain(after.cloned())
            .collect();
        *exprs = new_vec;
    }
    expand_happened |= !replacements.is_empty();
    Ok(expand_happened)
}

fn if_equal_replacement(expr: &SExpr) -> Result<Option<Vec<SExpr>>> {
    strings_compare_replacement(expr, "if-equal")
}

fn if_not_equal_replacement(expr: &SExpr) -> Result<Option<Vec<SExpr>>> {
    strings_compare_replacement(expr, "if-not-equal")
}

fn if_in_list_replacement(expr: &SExpr) -> Result<Option<Vec<SExpr>>> {
    string_list_compare_replacement(expr, "if-in-list")
}

fn if_not_in_list_replacement(expr: &SExpr) -> Result<Option<Vec<SExpr>>> {
    string_list_compare_replacement(expr, "if-not-in-list")
}

fn strings_compare_replacement(expr: &SExpr, operation: &str) -> Result<Option<Vec<SExpr>>> {
    match expr {
        // Below should not be reached because only lists should be visited
        SExpr::Atom(_) => unreachable!(),
        SExpr::List(l) => Ok(match l.t.first() {
            Some(SExpr::Atom(Spanned { t, .. })) if t.as_str() == operation => {
                let first =
                    l.t.get(1)
                        .ok_or_else(|| {
                            anyhow_expr!(
                                &expr,
                                "{operation} expects a string comparand as the first parameter"
                            )
                        })
                        .and_then(|expr| {
                            expr.atom(None).ok_or_else(|| {
                                anyhow_expr!(&expr, "comparands within {operation} must be strings")
                            })
                        })?;
                let second =
                    l.t.get(2)
                        .ok_or_else(|| {
                            anyhow_expr!(
                                &expr,
                                "{operation} expects a string comparand as the second parameter"
                            )
                        })
                        .and_then(|expr| {
                            expr.atom(None).ok_or_else(|| {
                                anyhow_expr!(&expr, "comparands within {operation} must be strings")
                            })
                        })?;
                if match operation {
                    "if-equal" => first == second,
                    "if-not-equal" => first != second,
                    _ => unreachable!(),
                } {
                    Some(l.t.iter().skip(3).cloned().collect())
                } else {
                    Some(vec![])
                }
            }
            _ => None,
        }),
    }
}

fn string_list_compare_replacement(expr: &SExpr, operation: &str) -> Result<Option<Vec<SExpr>>> {
    match expr {
        // Below should not be reached because only lists should be visited
        SExpr::Atom(_) => unreachable!(),
        SExpr::List(l) => Ok(match l.t.first() {
            Some(SExpr::Atom(Spanned { t, .. })) if t.as_str() == operation => {
                let first =
                    l.t.get(1)
                        .ok_or_else(|| {
                            anyhow_expr!(
                                &expr,
                                "{operation} expects a string comparand as the first parameter"
                            )
                        })
                        .and_then(|expr| {
                            expr.atom(None).ok_or_else(|| {
                                anyhow_expr!(
                                    &expr,
                                    "the first parameter of {operation} must be a string"
                                )
                            })
                        })?;
                let second =
                    l.t.get(2)
                        .ok_or_else(|| {
                            anyhow_expr!(
                                &expr,
                                "{operation} expects a list comparand as the second parameter"
                            )
                        })
                        .and_then(|expr| {
                            expr.list(None).ok_or_else(|| {
                                anyhow_expr!(
                                    &expr,
                                    "the second parameter of {operation} must be a list"
                                )
                            })
                        })?;
                let mut in_list = false;
                visit_validate_all_atoms(second, &mut |s| {
                    in_list |= s.t == first;
                    Ok(())
                })?;
                if match operation {
                    "if-in-list" => in_list,
                    "if-not-in-list" => !in_list,
                    _ => unreachable!(),
                } {
                    Some(l.t.iter().skip(3).cloned().collect())
                } else {
                    Some(vec![])
                }
            }
            _ => None,
        }),
    }
}
