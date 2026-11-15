use super::*;

use crate::bail_expr;

pub(crate) fn parse_vars(
    exprs: &[&Vec<SExpr>],
    _lsp_hints: &mut LspHints,
) -> Result<HashMap<String, SExpr>> {
    let mut vars: HashMap<String, SExpr> = Default::default();
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defvar")?;
        // Read k-v pairs from the configuration
        while let Some(var_name_expr) = subexprs.next() {
            let var_name = match var_name_expr {
                SExpr::Atom(a) => &a.t,
                _ => bail_expr!(var_name_expr, "variable name must not be a list"),
            };
            let var_expr = match subexprs.next() {
                Some(v) => match v {
                    SExpr::Atom(_) => v.clone(),
                    SExpr::List(l) => parse_list_var(l, &vars),
                },
                None => bail_expr!(var_name_expr, "variable name must have a subsequent value"),
            };
            check_var_cycle(var_name, &var_expr, &vars)?;
            #[cfg(feature = "lsp")]
            _lsp_hints
                .definition_locations
                .variable
                .insert(var_name.to_owned(), var_name_expr.span());
            if vars.insert(var_name.into(), var_expr).is_some() {
                bail_expr!(var_name_expr, "duplicate variable name: {}", var_name);
            }
        }
    }
    Ok(vars)
}

/// Errors if `var_expr`, the value about to be stored for `var_name`, refers back to `var_name`
/// through `$name` references, either directly or nested inside lists. Variables are resolved
/// when they are used, so a cycle would otherwise recurse until the stack overflows. Every
/// variable is checked when it is added, so the existing variables never contain a cycle.
fn check_var_cycle(var_name: &str, var_expr: &SExpr, vars: &HashMap<String, SExpr>) -> Result<()> {
    let mut visited = HashSet::default();
    if let Some(path) = find_var_ref_path(var_name, var_expr, vars, &mut visited) {
        bail_expr!(
            var_expr,
            "variable cycle: {var_name} -> {}",
            path.join(" -> ")
        );
    }
    Ok(())
}

/// Returns the variable names, ending with `target`, through which `expr` refers to `target`.
fn find_var_ref_path<'a>(
    target: &str,
    expr: &'a SExpr,
    vars: &'a HashMap<String, SExpr>,
    visited: &mut HashSet<&'a str>,
) -> Option<Vec<&'a str>> {
    match expr {
        SExpr::Atom(a) => {
            let name = a.t.strip_prefix('$')?;
            if name == target {
                return Some(vec![name]);
            }
            if !visited.insert(name) {
                return None;
            }
            let mut path = find_var_ref_path(target, vars.get(name)?, vars, visited)?;
            path.insert(0, name);
            Some(path)
        }
        SExpr::List(l) => {
            l.t.iter()
                .find_map(|e| find_var_ref_path(target, e, vars, visited))
        }
    }
}

pub(crate) fn parse_list_var(expr: &Spanned<Vec<SExpr>>, vars: &HashMap<String, SExpr>) -> SExpr {
    let ret = match expr.t.first() {
        Some(SExpr::Atom(a)) => match a.t.as_str() {
            "concat" => {
                let mut concat_str = String::new();
                let visitees = &expr.t[1..];
                push_all_atoms(visitees, vars, &mut concat_str);
                SExpr::Atom(Spanned {
                    span: expr.span.clone(),
                    t: concat_str,
                })
            }
            _ => SExpr::List(expr.clone()),
        },
        _ => SExpr::List(expr.clone()),
    };
    ret
}

pub(crate) fn push_all_atoms(exprs: &[SExpr], vars: &HashMap<String, SExpr>, pusheen: &mut String) {
    for expr in exprs {
        if let Some(a) = expr.atom(Some(vars)) {
            pusheen.push_str(a.trim_atom_quotes());
        } else if let Some(l) = expr.list(Some(vars)) {
            push_all_atoms(l, vars, pusheen);
        }
    }
}
