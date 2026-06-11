//! Lexer: source text -> tokens. Every token carries a byte span so that
//! diagnostics anywhere downstream can point at real source.

use crate::diag::{Diagnostic, Span};
use crate::syntax;

#[derive(Debug, Clone, PartialEq)]
pub enum TokKind {
    KwFn,
    KwPub,
    KwVal,
    KwVar,
    KwMutate,
    KwMove,
    KwView,
    KwStored,
    KwStruct,
    KwConst,
    KwReturn,
    KwLoop,
    KwUnsafe,
    Ident(String),
    Str(String),
    Int(i64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Arrow,
    Semi,
    Eq,
    Dot,
    Star,
    At,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokKind,
    pub span: Span,
}

/// A short, human description of a token, for error messages.
/// Never say "token" to a user; say what the thing is.
pub fn describe(kind: &TokKind) -> String {
    match kind {
        TokKind::KwFn => format!("the keyword `{}`", syntax::KW_FN),
        TokKind::KwPub => format!("the keyword `{}`", syntax::KW_PUB),
        TokKind::KwVal => format!("the keyword `{}`", syntax::KW_VAL),
        TokKind::KwVar => format!("the keyword `{}`", syntax::KW_VAR),
        TokKind::KwMutate => format!("the keyword `{}`", syntax::KW_MUTATE),
        TokKind::KwMove => format!("the keyword `{}`", syntax::KW_MOVE),
        TokKind::KwView => format!("the keyword `{}`", syntax::KW_VIEW),
        TokKind::KwStored => format!("the keyword `{}`", syntax::KW_STORED),
        TokKind::KwStruct => format!("the keyword `{}`", syntax::KW_STRUCT),
        TokKind::KwConst => format!("the keyword `{}`", syntax::KW_CONST),
        TokKind::KwReturn => format!("the keyword `{}`", syntax::KW_RETURN),
        TokKind::KwLoop => format!("the keyword `{}`", syntax::KW_LOOP),
        TokKind::KwUnsafe => format!("the keyword `{}`", syntax::KW_UNSAFE),
        TokKind::Ident(name) => format!("the name `{}`", name),
        TokKind::Str(_) => "a piece of quoted text".to_string(),
        TokKind::Int(_) => "a number".to_string(),
        TokKind::LParen => "`(`".to_string(),
        TokKind::RParen => "`)`".to_string(),
        TokKind::LBrace => "`{`".to_string(),
        TokKind::RBrace => "`}`".to_string(),
        TokKind::LBracket => "`[`".to_string(),
        TokKind::RBracket => "`]`".to_string(),
        TokKind::Colon => "`:`".to_string(),
        TokKind::Comma => "`,`".to_string(),
        TokKind::Arrow => "`->`".to_string(),
        TokKind::Semi => "`;`".to_string(),
        TokKind::Eq => "`=`".to_string(),
        TokKind::Dot => "`.`".to_string(),
        TokKind::Star => "`*`".to_string(),
        TokKind::At => "`@`".to_string(),
        TokKind::Eof => "the end of the file".to_string(),
    }
}

fn keyword(name: &str) -> Option<TokKind> {
    match name {
        s if s == syntax::KW_FN => Some(TokKind::KwFn),
        s if s == syntax::KW_PUB => Some(TokKind::KwPub),
        s if s == syntax::KW_VAL => Some(TokKind::KwVal),
        s if s == syntax::KW_VAR => Some(TokKind::KwVar),
        s if s == syntax::KW_MUTATE => Some(TokKind::KwMutate),
        s if s == syntax::KW_MOVE => Some(TokKind::KwMove),
        s if s == syntax::KW_VIEW => Some(TokKind::KwView),
        s if s == syntax::KW_STORED => Some(TokKind::KwStored),
        s if s == syntax::KW_STRUCT => Some(TokKind::KwStruct),
        s if s == syntax::KW_CONST => Some(TokKind::KwConst),
        s if s == syntax::KW_RETURN => Some(TokKind::KwReturn),
        s if s == syntax::KW_LOOP => Some(TokKind::KwLoop),
        s if s == syntax::KW_UNSAFE => Some(TokKind::KwUnsafe),
        _ => None,
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, Diagnostic> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let end = src.len();
    let at = |i: usize| -> char {
        if i < chars.len() {
            chars[i].1
        } else {
            '\0'
        }
    };
    let pos = |i: usize| -> usize {
        if i < chars.len() {
            chars[i].0
        } else {
            end
        }
    };

    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = at(i);

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Line comments (decision S5).
        if c == '/' && at(i + 1) == '/' {
            while i < chars.len() && at(i) != '\n' {
                i += 1;
            }
            continue;
        }

        let start = pos(i);
        match c {
            '(' => {
                toks.push(Token {
                    kind: TokKind::LParen,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            ')' => {
                toks.push(Token {
                    kind: TokKind::RParen,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '{' => {
                toks.push(Token {
                    kind: TokKind::LBrace,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '}' => {
                toks.push(Token {
                    kind: TokKind::RBrace,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '[' => {
                toks.push(Token {
                    kind: TokKind::LBracket,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            ']' => {
                toks.push(Token {
                    kind: TokKind::RBracket,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            ':' => {
                toks.push(Token {
                    kind: TokKind::Colon,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            ',' => {
                toks.push(Token {
                    kind: TokKind::Comma,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            ';' => {
                toks.push(Token {
                    kind: TokKind::Semi,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '=' => {
                toks.push(Token {
                    kind: TokKind::Eq,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '.' => {
                toks.push(Token {
                    kind: TokKind::Dot,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '*' => {
                toks.push(Token {
                    kind: TokKind::Star,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '@' => {
                toks.push(Token {
                    kind: TokKind::At,
                    span: Span::new(start, pos(i + 1)),
                });
                i += 1;
            }
            '-' if at(i + 1) == '>' => {
                toks.push(Token {
                    kind: TokKind::Arrow,
                    span: Span::new(start, pos(i + 2)),
                });
                i += 2;
            }
            '"' => {
                i += 1;
                let mut text = String::new();
                let mut closed = false;
                while i < chars.len() {
                    let ch = at(i);
                    if ch == '"' {
                        closed = true;
                        i += 1;
                        break;
                    }
                    if ch == '\n' {
                        break;
                    }
                    text.push(ch);
                    i += 1;
                }
                if !closed {
                    return Err(Diagnostic::error(
                        "E0002",
                        "this text never gets a closing quote".to_string(),
                        "a piece of text must start and end with a `\"` on the same line"
                            .to_string(),
                        "add a closing `\"` before the end of the line".to_string(),
                        Some(Span::new(start, pos(i))),
                    ));
                }
                toks.push(Token {
                    kind: TokKind::Str(text),
                    span: Span::new(start, pos(i)),
                });
            }
            c if c.is_ascii_digit() => {
                let mut n: i64 = 0;
                let mut overflow = false;
                while i < chars.len() && at(i).is_ascii_digit() {
                    let d = at(i) as i64 - '0' as i64;
                    n = match n.checked_mul(10).and_then(|v| v.checked_add(d)) {
                        Some(v) => v,
                        None => {
                            overflow = true;
                            n
                        }
                    };
                    i += 1;
                }
                let span = Span::new(start, pos(i));
                if overflow {
                    return Err(Diagnostic::error(
                        "E0007",
                        "this number is too big".to_string(),
                        "numbers currently top out at 9223372036854775807 (a 64-bit integer)"
                            .to_string(),
                        "use a smaller number".to_string(),
                        Some(span),
                    ));
                }
                toks.push(Token { kind: TokKind::Int(n), span });
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut name = String::new();
                while i < chars.len() {
                    let ch = at(i);
                    if ch.is_alphanumeric() || ch == '_' {
                        name.push(ch);
                        i += 1;
                    } else {
                        break;
                    }
                }
                let span = Span::new(start, pos(i));
                let kind = keyword(&name).unwrap_or(TokKind::Ident(name));
                toks.push(Token { kind, span });
            }
            other => {
                return Err(Diagnostic::error(
                    "E0001",
                    format!("the character `{}` doesn't mean anything here (yet)", other),
                    "check docs/01-spec.md for what's supported so far".to_string(),
                    "remove it, or use supported syntax".to_string(),
                    Some(Span::new(start, pos(i + 1))),
                ));
            }
        }
    }
    toks.push(Token {
        kind: TokKind::Eof,
        span: Span::new(end, end),
    });
    Ok(toks)
}
