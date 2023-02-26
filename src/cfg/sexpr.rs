use std::ops::Index;
use std::str::Bytes;
use std::{cmp, iter};

type ParseError = Spanned<String>;

type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    fn new(start: usize, end: usize) -> Span {
        assert!(start <= end);
        Span { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn cover(self, other: Span) -> Span {
        let start = cmp::min(self.start(), other.start());
        let end = cmp::max(self.end(), other.end());
        Span::new(start, end)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub fn atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(a) => Some(a.t.as_str()),
            _ => None,
        }
    }

    pub fn list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List(l) => Some(&l.t),
            _ => None,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            SExpr::Atom(a) => a.span,
            SExpr::List(l) => l.span,
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
                if !l.t.is_empty() {
                    write!(f, "{:?}", &l.t.last().unwrap())?;
                }
                write!(f, ")")?;
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
enum Token {
    Open,
    Close,
    StringTok,
}
pub struct Lexer<'a> {
    s: &'a str,
    bytes: Bytes<'a>,
}

fn is_start(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'"') || b.is_ascii_whitespace()
}

type TokenRes = Result<Token, String>;

impl<'a> Lexer<'a> {
    #[allow(clippy::new_ret_no_self)]
    fn new(s: &str) -> impl Iterator<Item = Spanned<TokenRes>> + '_ {
        let mut lexer = Lexer {
            s,
            bytes: s.bytes(),
        };
        iter::from_fn(move || {
            lexer
                .next_token()
                .map(|(start, t)| Spanned::new(t, Span::new(start, lexer.pos())))
        })
    }

    fn next_while(&mut self, f: impl Fn(u8) -> bool) {
        for b in self.bytes.clone() {
            if f(b) {
                self.bytes.next().unwrap();
            } else {
                break;
            }
        }
    }

    /// Looks for "|#", consuming bytes until found. If not found, returns Some(Err(...));
    /// otherwise returns None.
    fn read_until_multiline_comment_end(&mut self) -> Option<TokenRes> {
        let mut found_comment_end = false;
        for b2 in self.bytes.clone().skip(1) {
            let b1 = self.bytes.next().unwrap();
            if b1 == b'|' && b2 == b'#' {
                found_comment_end = true;
                break;
            }
        }
        if !found_comment_end {
            return Some(Err(
                "Unterminated multiline comment. Add |# after the end of your comment.".to_string(),
            ));
        }
        self.bytes.next();
        None
    }

    fn pos(&self) -> usize {
        self.s.len() - self.bytes.len()
    }

    fn next_token(&mut self) -> Option<(usize, TokenRes)> {
        use Token::*;
        loop {
            let start = self.pos();
            break match self.bytes.next() {
                Some(b) => Some((
                    start,
                    Ok(match b {
                        b'(' => Open,
                        b')' => Close,
                        b'"' => {
                            self.next_while(|b| b != b'"' && b != b'\n');
                            match self.bytes.next() {
                                Some(b'"') => StringTok,
                                _ => return Some((start, Err("Unterminated string".to_string()))),
                            }
                        }
                        b';' => match self.bytes.clone().next() {
                            Some(b';') => {
                                self.next_while(|b| b != b'\n');
                                // possibly consume the newline (or EOF handled in next iteration)
                                let _ = self.bytes.next();
                                continue;
                            }
                            _ => self.next_string(),
                        },
                        b'#' => match self.bytes.clone().next() {
                            Some(b'|') => {
                                // consume the '|'
                                self.bytes.next();
                                if let Some(e) = self.read_until_multiline_comment_end() {
                                    return Some((start, e));
                                }
                                continue;
                            }
                            _ => self.next_string(),
                        },
                        b if b.is_ascii_whitespace() => {
                            self.next_while(|b| b.is_ascii_whitespace());
                            continue;
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
}

type TopLevel = Spanned<Vec<SExpr>>;

pub fn parse(cfg: &str) -> Result<Vec<TopLevel>, (String, usize, usize)> {
    parse_(cfg).map_err(transform_error)
}

fn parse_(s: &str) -> ParseResult<Vec<TopLevel>> {
    parse_with(s, Lexer::new(s))
}

fn parse_with(
    s: &str,
    mut tokens: impl Iterator<Item = Spanned<TokenRes>>,
) -> ParseResult<Vec<TopLevel>> {
    use SExpr::*;
    use Token::*;
    let mut stack = vec![Spanned::new(vec![], Span::new(0, 0))];
    loop {
        match tokens.next() {
            None => break,
            Some(Spanned { t, span }) => match t.map_err(|s| Spanned::new(s, span))? {
                Open => stack.push(Spanned::new(vec![], span)),
                Close => {
                    let Spanned {
                        t: exprs,
                        span: stack_span,
                    } = stack.pop().unwrap();
                    let expr = List(Spanned::new(exprs, stack_span.cover(span)));
                    if stack.is_empty() {
                        return Err(Spanned::new(
                            "Unexpected closing parenthesis".to_string(),
                            span,
                        ));
                    }
                    stack.last_mut().unwrap().t.push(expr);
                }
                StringTok => stack
                    .last_mut()
                    .unwrap()
                    .t
                    .push(Atom(Spanned::new(s[span].to_string(), span))),
            },
        }
    }
    let Spanned { t: exprs, span: sp } = stack.pop().unwrap();
    if !stack.is_empty() {
        return Err(Spanned::new("Unclosed parenthesis".to_string(), sp));
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
    Ok(exprs)
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

/// Returns the error message, start and length.
fn transform_error(e: ParseError) -> (String, usize, usize) {
    let start = e.span.start();
    let end = e.span.end();
    let mut len = end - start;
    if e.t.contains("Unterminated multiline comment") {
        len = 2;
    };
    (e.t, start, len)
}

#[test]
fn span_works() {
    let s = "(hello world my oyster)\n(row two)";
    let tlevel = parse(s).unwrap();
    assert_eq!(
        &s[tlevel[0].span.start..tlevel[0].span.end],
        "(hello world my oyster)"
    );
    assert_eq!(&s[tlevel[1].span.start..tlevel[1].span.end], "(row two)");
}
