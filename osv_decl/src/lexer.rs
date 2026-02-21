use crate::diagnostics::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Keyword(String),
    Ident(String),
    StringLit(String),
    IntLit(u32),
    LBrace,
    RBrace,
    LParen,
    RParen,
    Comma,
    Semi,
    Star,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut chars = source.chars().peekable();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut col = 1usize;

    while let Some(ch) = chars.peek().copied() {
        if ch == '\n' {
            chars.next();
            line += 1;
            col = 1;
            continue;
        }

        if ch.is_whitespace() {
            chars.next();
            col += 1;
            continue;
        }

        if ch == '/' {
            let span = Span::new(line, col);
            chars.next();
            col += 1;
            if chars.peek() == Some(&'/') {
                while let Some(c) = chars.peek().copied() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                    col += 1;
                }
                continue;
            }
            return Err(Diagnostic::new(
                "E1001",
                "unexpected '/' (use // for comments)",
                Some(span),
            ));
        }

        let span = Span::new(line, col);
        match ch {
            '{' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::LBrace,
                    span,
                });
            }
            '}' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::RBrace,
                    span,
                });
            }
            '(' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::LParen,
                    span,
                });
            }
            ')' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::RParen,
                    span,
                });
            }
            ',' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::Comma,
                    span,
                });
            }
            ';' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::Semi,
                    span,
                });
            }
            '*' => {
                chars.next();
                col += 1;
                out.push(Token {
                    kind: TokenKind::Star,
                    span,
                });
            }
            '"' => {
                chars.next();
                col += 1;
                let mut s = String::new();
                let mut escaped = false;
                loop {
                    let Some(c) = chars.next() else {
                        return Err(Diagnostic::new(
                            "E1002",
                            "unterminated string literal",
                            Some(span),
                        ));
                    };
                    col += 1;
                    if escaped {
                        s.push(c);
                        escaped = false;
                        continue;
                    }
                    if c == '\\' {
                        escaped = true;
                        continue;
                    }
                    if c == '"' {
                        break;
                    }
                    if c == '\n' {
                        return Err(Diagnostic::new(
                            "E1003",
                            "newline in string literal",
                            Some(span),
                        ));
                    }
                    s.push(c);
                }
                out.push(Token {
                    kind: TokenKind::StringLit(s),
                    span,
                });
            }
            c if c.is_ascii_digit() => {
                let mut s = String::new();
                while let Some(c2) = chars.peek().copied() {
                    if c2.is_ascii_digit() {
                        s.push(c2);
                        chars.next();
                        col += 1;
                    } else {
                        break;
                    }
                }
                let value = s.parse::<u32>().map_err(|_| {
                    Diagnostic::new("E1004", "integer literal out of range", Some(span))
                })?;
                out.push(Token {
                    kind: TokenKind::IntLit(value),
                    span,
                });
            }
            c if is_ident_start(c) => {
                let mut s = String::new();
                while let Some(c2) = chars.peek().copied() {
                    if is_ident_continue(c2) {
                        s.push(c2);
                        chars.next();
                        col += 1;
                    } else {
                        break;
                    }
                }
                let kind = if is_keyword(&s) {
                    TokenKind::Keyword(s)
                } else {
                    TokenKind::Ident(s)
                };
                out.push(Token { kind, span });
            }
            _ => {
                return Err(Diagnostic::new(
                    "E1005",
                    format!("unexpected character '{}'", ch),
                    Some(span),
                ));
            }
        }
    }

    out.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(line, col),
    });
    Ok(out)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_keyword(value: &str) -> bool {
    match value {
        "app" | "version" | "role" | "inherits" | "can" | "layer" | "as" | "path" | "namespace"
        | "grant" | "shard" | "by" | "cache" | "allow" | "to" | "on" | "derive" | "from"
        | "using" | "ui_app" | "entry" | "shared" | "creator" | "open" | "explicit"
        | "role_scoped" | "map" | "list" | "text" | "counter" | "blob" | "create" | "read"
        | "write" | "sync" | "revoke" | "minute" | "hour" | "day" | "week" | "month"
        | "manage_access" | "accept_publish" | "delegate" | "share" | "relay" | "ui_window"
        | "memory_lru" | "sync_mode" | "full_snapshot" | "incremental" | "retention_days" => true,
        _ => false,
    }
}
