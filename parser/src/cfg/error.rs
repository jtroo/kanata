use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use super::{sexpr::Span, *};

pub type MResult<T> = miette::Result<T>;
pub type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub msg: String,
    pub span: Option<Span>,
}

impl ParseError {
    pub fn new(span: Span, err_msg: impl AsRef<str>) -> Self {
        Self {
            msg: err_msg.as_ref().to_string(),
            span: Some(span),
        }
    }

    pub fn new_without_span(err_msg: impl AsRef<str>) -> Self {
        Self {
            msg: err_msg.as_ref().to_string(),
            span: None,
        }
    }

    pub fn from_expr(expr: &sexpr::SExpr, err_msg: impl AsRef<str>) -> Self {
        Self::new(expr.span(), err_msg)
    }

    pub fn from_spanned<T>(spanned: &Spanned<T>, err_msg: impl AsRef<str>) -> Self {
        Self::new(spanned.span.clone(), err_msg)
    }
}

impl From<anyhow::Error> for ParseError {
    fn from(value: anyhow::Error) -> Self {
        Self::new_without_span(value.to_string())
    }
}

impl From<ParseError> for miette::Error {
    fn from(val: ParseError) -> Self {
        let diagnostic = CfgError {
            err_span: val
                .span
                .as_ref()
                .map(|s| SourceSpan::new(s.start().into(), (s.end() - s.start()).into())),
            help_msg: help(val.msg),
            file_name: val.span.as_ref().map(|s| s.file_name()),
            file_content: val.span.as_ref().map(|s| s.file_content()),
        };

        let report: miette::Error = diagnostic.into();

        if let Some(span) = val.span {
            report.with_source_code(NamedSource::new(span.file_name(), span.file_content()))
        } else {
            report
        }
    }
}

#[derive(Error, Debug, Diagnostic, Clone)]
#[error("Error in configuration")]
#[diagnostic()]
struct CfgError {
    // Snippets and highlights can be included in the diagnostic!
    #[label("Error here")]
    err_span: Option<SourceSpan>,
    #[help]
    help_msg: String,
    file_name: Option<String>,
    file_content: Option<String>,
}

pub(super) fn help(err_msg: impl AsRef<str>) -> String {
    format!(
        r"{}

For more info, see the configuration guide:
https://github.com/jtroo/kanata/blob/main/docs/config.adoc",
        err_msg.as_ref(),
    )
}
