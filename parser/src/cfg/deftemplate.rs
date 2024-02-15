use crate::anyhow_expr;
use crate::anyhow_span;
use crate::bail_span;

use super::*;
use super::error::*;
use super::sexpr::*;

#[derive(Debug)]
pub struct Template {
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

    for list in toplevel_exprs.iter_mut() {
        if !matches!(list.t.first().and_then(|expr| expr.atom(None)), Some("deftemplate")) {
            continue;
        }
        let vars = list.t.iter().nth(1)
            .ok_or_else(|| anyhow_span!(list, "deftemplate must have a list as the first parameter"))
            .and_then(|v| v.list(None).ok_or_else(|| anyhow_expr!(v, "deftemplate must have a list as the first parameter")))
            .and_then(|v| v.iter().try_fold(vec![], |mut vars, var| {
                let s = var.atom(None).map(|a| a.to_owned()).ok_or_else(|| anyhow_expr!(var, "deftemplate variables must be strings"))?;
                vars.push(s);
                Ok(vars)
            }))?;
        
        let content: Vec<SExpr> = list.t.iter().skip(2).cloned().collect();

        let mut var_usage_counts: HashMap<String, u32> = vars.iter().map(|v| (v.clone(), 0)).collect();
        visit_validate_all_atoms(&content, |s| {
            match s.t.as_str() {
                "deftemplate" => bail_span!(s, "deftemplate is not allowed within deftemplate"),
                "template-expand" => bail_span!(s, "template-expand is not allowed within deftemplate"),
                s => {
                    if let Some(count) = var_usage_counts.get_mut(s) {
                        *count += 1;
                    }
                    Ok(())
                }
            }
        });
        for (var, count) in var_usage_counts.iter() {
            if *count == 0 {
                log::warn!("deftemplate variable {var} did not appear in its template");
            }
        }
        templates.push(Template {vars, content});
    }
    // - single pass of deftemplate parsing at top-level
    //   - error if deftemplate or template-expand found within deftemplate
    // - single pass all-depths search for template expansion
    //   - error if deftemplate found outside of top-level
    //   - 
    todo!()
}

fn visit_validate_all_atoms(exprs: &[SExpr], mut visit: impl FnMut(&Spanned<String>) -> Result<()>) -> Result<()> {
    for expr in exprs {
        match expr {
            SExpr::Atom(a) => visit(a)?,
            SExpr::List(l) => visit_validate_all_atoms(&l.t, visit)?,
        }
    }
    Ok(())
}