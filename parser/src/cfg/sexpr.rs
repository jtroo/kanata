use std::iter;
use std::ops::Index;
use std::rc::Rc;
use std::str::Bytes;

type ParseError = Spanned<String>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

type ParseResult<T> = Result<T, ParseError>;

use super::error::{span_start_len, CfgError};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Position {
    /// The position since the beginning of the file, in bytes.
    pub absolute: usize,
    /// The number of newline characters since the beginning of the file.
    pub line: usize,
    /// The position since the beginning of line, in bytes, 0-indexed.
    pub column: usize, // TODO: figure out alternative (because of grapheme clusters vs bytes vs codepoints can produce different column pos)
}

impl Position {
    fn new(absolute: usize, line: usize, column: usize) -> Self {
        assert!(line <= absolute);
        assert!(column <= absolute);
        Self {
            absolute,
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: Position,
    pub end: Position,
    pub file_name: Rc<str>,
    pub file_content: Rc<str>,
}

impl Default for Span {
    fn default() -> Self {
        Self {
            start: Position::default(),
            end: Position::default(),
            file_name: Rc::from(""),
            file_content: Rc::from(""),
        }
    }
}

impl Span {
    fn new(start: Position, end: Position, file_name: Rc<str>, file_content: Rc<str>) -> Span {
        assert!(start.absolute <= end.absolute);
        assert!(start.line <= end.line);
        Span {
            start,
            end,
            file_name,
            file_content,
        }
    }

    pub fn cover(&self, other: &Span) -> Span {
        assert!(self.file_name == other.file_name);

        let start: Position;
        if self.start() <= other.start() {
            start = self.start.clone();
        } else {
            start = other.start.clone();
        }

        let end: Position;
        if self.end() >= other.end() {
            end = self.end.clone();
        } else {
            end = other.end.clone();
        }

        Span::new(
            start,
            end,
            self.file_name.clone(),
            self.file_content.clone(),
        )
    }

    pub fn start(&self) -> usize {
        self.start.absolute
    }

    pub fn end(&self) -> usize {
        self.end.absolute
    }

    pub fn file_name(&self) -> String {
        self.file_name.clone().to_string()
    }

    pub fn file_content(&self) -> String {
        self.file_content.clone().to_string()
    }
}

impl Index<Span> for str {
    type Output = str;
    fn index(&self, span: Span) -> &Self::Output {
        &self[span.start()..span.end()]
    }
}

impl Index<Span> for String {
    type Output = str;
    fn index(&self, span: Span) -> &Self::Output {
        &self[span.start()..span.end()]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spanned<T> {
    pub t: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(t: T, span: Span) -> Spanned<T> {
        Spanned { t, span }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// I know this isn't the classic definition of an S-Expression which uses cons cell and atom, but
/// this is more convenient to work with (I find).
pub enum SExpr {
    Atom(Spanned<String>),
    List(Spanned<Vec<SExpr>>),
}

impl SExpr {
    pub fn atom<'a>(&'a self, vars: Option<&'a HashMap<String, SExpr>>) -> Option<&'a str> {
        match self {
            SExpr::Atom(a) => {
                let s = a.t.as_str();
                match (s.strip_prefix('$'), vars) {
                    (Some(varname), Some(vars)) => match vars.get(varname) {
                        Some(var) => var.atom(Some(vars)),
                        None => Some(s),
                    },
                    _ => Some(s),
                }
            }
            _ => None,
        }
    }

    pub fn list<'a>(&'a self, vars: Option<&'a HashMap<String, SExpr>>) -> Option<&'a [SExpr]> {
        match self {
            SExpr::List(l) => Some(&l.t),
            SExpr::Atom(a) => match (a.t.strip_prefix('$'), vars) {
                (Some(varname), Some(vars)) => match vars.get(varname) {
                    Some(var) => var.list(Some(vars)),
                    None => None,
                },
                _ => None,
            },
        }
    }

    pub fn span(&self) -> Span {
        match self {
            SExpr::Atom(a) => a.span.clone(),
            SExpr::List(l) => l.span.clone(),
        }
    }
}

impl std::fmt::Debug for SExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SExpr::Atom(a) => write!(f, "{}", &a.t),
            SExpr::List(l) => {
                write!(f, "(")?;
                for i in 0..l.t.len() - 1 {
                    write!(f, "{:?} ", &l.t[i])?;
                }
                if let Some(last) = &l.t.last() {
                    write!(f, "{last:?}")?;
                }
                write!(f, ")")?;
                Ok(())
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Complementary to SExpr metadata items.
pub enum SExprMetaData {
    LineComment(Spanned<String>),
    BlockComment(Spanned<String>),
    Whitespace(Spanned<String>),
}

#[derive(Debug)]
enum Token {
    Open,
    Close,
    StringTok,
    BlockComment,
    LineComment,
    Whitespace,
}

pub struct Lexer<'a> {
    s: &'a str,
    bytes: Bytes<'a>,
    ignore_whitespace_and_comments: bool,
    line: usize,
    last_newline_pos: usize,
}

fn is_start(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'"') || b.is_ascii_whitespace()
}

type TokenRes = Result<Token, String>;

impl<'a> Lexer<'a> {
    #[allow(clippy::new_ret_no_self)]
    /// `file_name` is used only for indicating a file, where
    /// a fragment of `source` that caused parsing error came from.
    fn new(
        source: &'a str,
        file_name: &'a str,
        ignore_whitespace_and_comments: bool,
    ) -> impl Iterator<Item = Spanned<TokenRes>> + 'a {
        let mut lexer = Lexer {
            s: source,
            bytes: source.bytes(),
            ignore_whitespace_and_comments,
            line: 0,
            last_newline_pos: 0,
        };
        let file_name: Rc<str> = Rc::from(file_name);
        let file_content: Rc<str> = Rc::from(source);
        iter::from_fn(move || {
            lexer.next_token().map(|(start, t)| {
                let end = lexer.pos();
                // log::debug!(
                //     "TOKEN: {:?}: `{}`",
                //     t.as_ref().unwrap_or(&Token::Error),
                //     &file_content[start.absolute..end.absolute]
                // );
                Spanned::new(
                    t,
                    Span::new(start, end, file_name.clone(), file_content.clone()),
                )
            })
        })
    }

    fn next_while(&mut self, f: impl Fn(u8) -> bool) {
        for b in self.bytes.clone() {
            if f(b) {
                // Iterating over a clone of this iterator - this is guaranteed to be Some
                self.bytes.next().expect("iter lag");
                if b == b'\n' {
                    self.last_newline_pos = self.pos().absolute - 1;
                    self.line += 1;
                }
            } else {
                break;
            }
        }
    }

    /// Looks for "|#", consuming bytes until found. If not found, returns Err(...);
    fn read_until_multiline_comment_end(&mut self) -> TokenRes {
        for b2 in self.bytes.clone().skip(1) {
            // Iterating over a clone of this iterator that's 1 item ahead - this is guaranteed to
            // be Some.
            let b1 = self.bytes.next().expect("iter lag");
            if b1 == b'\n' {
                self.last_newline_pos = self.pos().absolute - 1;
                self.line += 1;
            }
            if b1 == b'|' && b2 == b'#' {
                self.bytes.next();
                return Ok(Token::BlockComment);
            }
        }
        Err("Unterminated multiline comment. Add |# after the end of your comment.".to_string())
    }

    fn pos(&self) -> Position {
        let absolute = self.s.len() - self.bytes.len();
        Position::new(absolute, self.line, absolute - self.last_newline_pos)
    }

    fn next_token(&mut self) -> Option<(Position, TokenRes)> {
        use Token::*;
        loop {
            let start = self.pos();
            break match self.bytes.next() {
                Some(b) => Some((
                    start.clone(),
                    Ok(match b {
                        b'(' => Open,
                        b')' => Close,
                        b'"' => {
                            self.next_while(|b| b != b'"' && b != b'\n');
                            match self.bytes.next() {
                                Some(b'"') => StringTok,
                                _ => {
                                    return Some((
                                        start.clone(),
                                        Err("Unterminated string".to_string()),
                                    ))
                                }
                            }
                        }
                        b';' => match self.bytes.clone().next() {
                            Some(b';') => {
                                self.next_while(|b| b != b'\n');
                                // possibly consume the newline (or EOF handled in next iteration)
                                if let Some(b2) = self.bytes.next() {
                                    if b2 == b'\n' {
                                        self.last_newline_pos = self.pos().absolute - 1;
                                        self.line += 1;
                                    }
                                }
                                if self.ignore_whitespace_and_comments {
                                    continue;
                                }
                                Token::LineComment
                            }
                            _ => self.next_string(),
                        },
                        b'#' => match self.bytes.clone().next() {
                            Some(b'|') => {
                                // consume the '|'
                                self.bytes.next();
                                let tok: Token = match self.read_until_multiline_comment_end() {
                                    Ok(t) => t,
                                    e @ Err(_) => return Some((start.clone(), e)),
                                };
                                if self.ignore_whitespace_and_comments {
                                    continue;
                                }
                                tok
                            }
                            _ => self.next_string(),
                        },
                        b if b.is_ascii_whitespace() => {
                            if b == b'\n' {
                                self.last_newline_pos = self.pos().absolute - 1;
                                self.line += 1;
                            }
                            let tok = self.next_whitespace();
                            if self.ignore_whitespace_and_comments {
                                continue;
                            }
                            tok
                        }
                        _ => self.next_string(),
                    }),
                )),
                None => None,
            };
        }
    }

    fn next_string(&mut self) -> Token {
        // might want to limit this to ascii or XID_START/XID_CONTINUE
        self.next_while(|b| !is_start(b));
        Token::StringTok
    }

    fn next_whitespace(&mut self) -> Token {
        self.next_while(|b| b.is_ascii_whitespace());
        Token::Whitespace
    }
}

pub type TopLevel = Spanned<Vec<SExpr>>;

pub fn parse(cfg: &str, file_name: &str) -> Result<Vec<TopLevel>, CfgError> {
    let ignore_whitespace_and_comments = true;
    parse_(cfg, file_name, ignore_whitespace_and_comments)
        .map_err(transform_error)
        .map(|(x, _)| x)
}

pub fn parse_(
    cfg: &str,
    file_name: &str,
    ignore_whitespace_and_comments: bool,
) -> ParseResult<(Vec<TopLevel>, Vec<SExprMetaData>)> {
    parse_with(
        cfg,
        Lexer::new(cfg, file_name, ignore_whitespace_and_comments),
    )
}

fn parse_with(
    s: &str,
    mut tokens: impl Iterator<Item = Spanned<TokenRes>>,
) -> ParseResult<(Vec<TopLevel>, Vec<SExprMetaData>)> {
    use Token::*;
    let mut stack = vec![Spanned::new(vec![], Span::default())];
    let mut metadata: Vec<SExprMetaData> = vec![];
    loop {
        match tokens.next() {
            None => break,
            Some(Spanned { t, span }) => match t.map_err(|s| Spanned::new(s, span.clone()))? {
                Open => stack.push(Spanned::new(vec![], span.clone())),
                Close => {
                    let Spanned {
                        t: exprs,
                        span: stack_span,
                        // There is a placeholder at the bottom of the stack to allow this unwrap;
                        // if the stack is ever empty, return an error.
                    } = stack.pop().expect("placeholder unpopped");
                    if stack.is_empty() {
                        return Err(Spanned::new(
                            "Unexpected closing parenthesis".to_string(),
                            span,
                        ));
                    }
                    let expr = SExpr::List(Spanned::new(exprs, stack_span.cover(&span.clone())));
                    stack.last_mut().expect("not empty").t.push(expr);
                }
                StringTok => {
                    stack
                        .last_mut()
                        .expect("not empty")
                        .t
                        .push(SExpr::Atom(Spanned::new(
                            s[span.clone()].to_string(),
                            span.clone(),
                        )))
                }
                BlockComment => metadata.push(SExprMetaData::BlockComment(Spanned::new(
                    s[span.clone()].to_string(),
                    span.clone(),
                ))),
                LineComment => metadata.push(SExprMetaData::LineComment(Spanned::new(
                    s[span.clone()].to_string(),
                    span.clone(),
                ))),
                Whitespace => metadata.push(SExprMetaData::Whitespace(Spanned::new(
                    s[span.clone()].to_string(),
                    span.clone(),
                ))),
            },
        }
    }
    // There is a placeholder at the bottom of the stack to allow this unwrap; if the stack is ever
    // empty, return an error.
    let Spanned { t: exprs, span: sp } = stack.pop().expect("placeholder unpopped");
    if !stack.is_empty() {
        return Err(Spanned::new("Unclosed opening parenthesis".to_string(), sp));
    }
    let exprs = exprs
        .into_iter()
        .map(|expr| match expr {
            SExpr::List(es) => Ok(es),
            SExpr::Atom(s) => Err(Spanned::new(
                "Everything must be in a list".to_string(),
                s.span,
            )),
        })
        .collect::<ParseResult<_>>()?;
    Ok((exprs, metadata))
}

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Error in configuration file syntax")]
#[diagnostic()]
pub struct LexError {
    // Snippets and highlights can be included in the diagnostic!
    #[label("Here")]
    pub err_span: SourceSpan,
    #[help]
    pub help_msg: String,
}

pub fn transform_error(e: ParseError) -> CfgError {
    let start = e.span.start();
    let end = e.span.end();
    let mut len = end - start;
    if e.t.contains("Unterminated multiline comment") {
        len = 2;
    };

    CfgError {
        err_span: Some(span_start_len(start, len)),
        help_msg: e.t,
        file_name: Some(e.span.file_name()),
        file_content: Some(e.span.file_content()),
    }
}
