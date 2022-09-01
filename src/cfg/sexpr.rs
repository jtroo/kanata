use std::fmt::Write;
use std::ops::Index;
use std::str::Bytes;
use std::{cmp, iter};

use anyhow::anyhow;

type ParseError = Spanned<String>;

type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    start: usize,
    end: usize,
}

pub struct LineCol {
    pub line: u32,
    pub col: u32,
}

impl LineCol {
    pub fn new(line: u32, col: u32) -> LineCol {
        LineCol { line, col }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spanned<T> {
    pub t: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    fn new(t: T, span: Span) -> Spanned<T> {
        Spanned { t, span }
    }
}

#[derive(Clone, Debug)]
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
}

#[derive(Debug)]
enum Token {
    Open,
    Close,
    String,
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
        while let Some(b) = self.bytes.clone().next() {
            if f(b) {
                self.bytes.next().unwrap();
            } else {
                break;
            }
        }
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
                                Some(b'"') => String,
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
        Token::String
    }
}

type TopLevel = Spanned<Vec<SExpr>>;

pub fn parse(s: &str) -> anyhow::Result<Vec<TopLevel>> {
    parse_(s).map_err(|e| anyhow!(pretty_errors(s, vec![e])))
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
                    let Spanned { t: exprs, span } = stack.pop().unwrap();
                    let expr = List(Spanned::new(exprs, span.cover(span)));
                    if stack.is_empty() {
                        return Err(Spanned::new(
                            "Unexpected closing parenthesis".to_string(),
                            span,
                        ));
                    }
                    stack.last_mut().unwrap().t.push(expr);
                }
                String => stack
                    .last_mut()
                    .unwrap()
                    .t
                    .push(Atom(Spanned::new(s[span].to_string(), span))),
            },
        }
    }
    let Spanned { t: exprs, .. } = stack.pop().unwrap();
    if !stack.is_empty() {
        return Err(Spanned::new(
            format!("{} Unclosed parentheses", stack.len()),
            Span::new(s.len().saturating_sub(1), s.len()),
        ));
        // bail!("Unclosed parens");
    }
    let exprs = exprs
        .into_iter()
        .map(|expr| match expr {
            SExpr::List(es) => Ok(es),
            SExpr::Atom(s) => Err(Spanned::new("Top level must be lists".to_string(), s.span)),
        })
        .collect::<ParseResult<_>>()?;
    Ok(exprs)
}

struct LineIndex {
    newlines: Vec<usize>,
}

impl LineIndex {
    pub fn new(s: &str) -> LineIndex {
        let mut newlines = vec![0];
        let mut curr_row = 0;
        for c in s.chars() {
            let c_len = c.len_utf8();
            curr_row += c_len;
            if c == '\n' {
                newlines.push(curr_row);
            }
        }

        LineIndex { newlines }
    }

    pub fn line_col(&self, offset: usize) -> LineCol {
        let line = self.newlines.partition_point(|&it| it <= offset) - 1;
        let line_start_offset = self.newlines[line];
        let col = offset - line_start_offset;
        LineCol::new(line as u32, col as u32)
    }

    pub fn get_line<'a>(&self, s: &'a str, line: usize) -> Option<&'a str> {
        let &off = self.newlines.get(line)?;
        Some(
            self.newlines
                .get(line + 1)
                .map(|off2| &s[off..off2 - 1])
                .unwrap_or_else(|| &s[off..]),
        )
    }
}

fn pretty_error(line_index: &LineIndex, s: &str, e: ParseError) -> String {
    let line_col_start = line_index.line_col(e.span.start());
    let line_col_end = line_index.line_col(e.span.end());
    let padding = line_col_end.line.to_string().len();
    let mut res = format!("error:\n{}\n", e.t);
    for line_num in line_col_start.line..line_col_end.line {
        let line = line_index.get_line(s, line_num as usize).unwrap();
        let padding = " ".repeat(padding - line_num.to_string().len());
        write!(res, "{line_num}{padding} | {line}").unwrap();
    }
    res
}

pub fn pretty_errors(s: &str, mut errors: Vec<ParseError>) -> String {
    let line_index = LineIndex::new(s);
    errors.sort_by_key(|t| t.span);
    errors
        .into_iter()
        .map(|e| format!("{}\n\n", pretty_error(&line_index, s, e)))
        .collect()
}
