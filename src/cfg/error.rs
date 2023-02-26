use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use super::*;

pub type MResult<T> = miette::Result<T>;
pub type Result<T> = std::result::Result<T, CfgError>;

#[derive(Error, Debug, Diagnostic)]
#[error("Error in configuration file")]
#[diagnostic()]
pub struct CfgError {
    // Snippets and highlights can be included in the diagnostic!
    #[label("Error here")]
    pub err_span: Option<SourceSpan>,
    #[help]
    pub help_msg: String,
}

pub(super) fn help(err_msg: impl AsRef<str>) -> String {
    format!(
        r"{}

For more info, see the configuration guide or ask in GitHub discussions.
    guide : https://github.com/jtroo/kanata/blob/main/docs/config.adoc
    ask   : https://github.com/jtroo/kanata/discussions",
        err_msg.as_ref(),
    )
}

pub(super) fn error_expr(expr: &sexpr::SExpr, err_msg: impl AsRef<str>) -> CfgError {
    CfgError {
        err_span: Some(expr_err_span(expr)),
        help_msg: help(err_msg),
    }
}

pub(super) fn error_spanned<T>(expr: &Spanned<T>, err_msg: impl AsRef<str>) -> CfgError {
    CfgError {
        err_span: Some(spanned_err_span(expr)),
        help_msg: help(err_msg),
    }
}

pub(super) fn span_start_len(start: usize, len: usize) -> SourceSpan {
    SourceSpan::new(start.into(), len.into())
}

pub(super) fn expr_err_span(expr: &sexpr::SExpr) -> SourceSpan {
    let span = expr.span();
    SourceSpan::new(span.start.into(), (span.end - span.start).into())
}

pub(super) fn spanned_err_span<T>(spanned: &Spanned<T>) -> SourceSpan {
    let span = spanned.span;
    SourceSpan::new(span.start.into(), (span.end - span.start).into())
}

pub(super) fn error_with_source(e: miette::Error, ps: &ParsedState) -> miette::Error {
    e.with_source_code(NamedSource::new(
        ps.cfg_filename.clone(),
        ps.cfg_text.clone(),
    ))
}

impl From<anyhow::Error> for CfgError {
    fn from(value: anyhow::Error) -> Self {
        Self {
            err_span: None,
            help_msg: help(value.to_string()),
        }
    }
}
