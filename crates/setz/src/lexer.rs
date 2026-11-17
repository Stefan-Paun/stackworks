//! Lexer: source text to tokens with 1-based line and column (columns count characters).

use crate::{Error, Pos};

/// Kind of a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    /// Integer literal, already reduced to its 32 bits.
    Int(i32),
    Ident(String),
    /// Keyword or operator/punctuation, spelled as in the source.
    Sym(&'static str),
    Eof,
}

/// A token and where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

/// Reserved words of the language.
pub const KEYWORDS: &[&str] = &[
    "fn", "var", "if", "else", "while", "break", "continue", "return", "read", "print", "putc",
];

/// Operators, longest first so that `<=` wins over `<`.
const OPERATORS: &[&str] = &[
    "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "(", ")", "{", "}", ",", ";", "=", "+", "-",
    "*", "/", "%", "<", ">", "&", "^", "|", "!", "~",
];

/// Splits `src` into tokens; the last token is always [`Tok::Eof`].
pub fn tokenize(src: &str) -> Result<Vec<Token>, Error> {
    let src = src.strip_prefix('\u{FEFF}').unwrap_or(src);
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1u32, 1u32);
    while i < chars.len() {
        let c = chars[i];
        let pos = Pos { line, col };
        if c == '\n' {
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c == ' ' || c == '\t' || c == '\r' {
            i += 1;
            col += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            col += (i - start) as u32;
            let tok = match KEYWORDS.iter().find(|k| **k == word) {
                Some(k) => Tok::Sym(k),
                None => Tok::Ident(word),
            };
            out.push(Token { tok, pos });
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            col += (i - start) as u32;
            let v = parse_int(&text)
                .ok_or_else(|| Error::new(pos, format!("invalid number '{text}'")))?;
            out.push(Token {
                tok: Tok::Int(v),
                pos,
            });
            continue;
        }
        if c == '\'' {
            let (v, len) = char_literal(&chars[i..])
                .ok_or_else(|| Error::new(pos, "invalid character literal".to_string()))?;
            i += len;
            col += len as u32;
            out.push(Token {
                tok: Tok::Int(v),
                pos,
            });
            continue;
        }
        let rest = &chars[i..];
        let op = OPERATORS.iter().find(|op| {
            let n = op.len();
            rest.len() >= n && op.chars().zip(rest).all(|(a, b)| a == *b)
        });
        match op {
            Some(op) => {
                i += op.len();
                col += op.len() as u32;
                out.push(Token {
                    tok: Tok::Sym(op),
                    pos,
                });
            }
            None => {
                let shown = if c.is_control() {
                    format!("U+{:04X}", c as u32)
                } else {
                    c.to_string()
                };
                return Err(Error::new(pos, format!("unexpected character '{shown}'")));
            }
        }
    }
    out.push(Token {
        tok: Tok::Eof,
        pos: Pos { line, col },
    });
    Ok(out)
}

/// Decimal, `0x` hexadecimal or `0b` binary, at most 2³² − 1, read as the i32 with the same bits.
fn parse_int(text: &str) -> Option<i32> {
    let (digits, radix) = if let Some(h) = text.strip_prefix("0x").or(text.strip_prefix("0X")) {
        (h, 16)
    } else if let Some(b) = text.strip_prefix("0b").or(text.strip_prefix("0B")) {
        (b, 2)
    } else {
        (text, 10)
    };
    if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') {
        return None;
    }
    let mut v: u64 = 0;
    for ch in digits.chars() {
        if ch == '_' {
            continue;
        }
        let d = ch.to_digit(radix)?;
        v = v * radix as u64 + d as u64;
        if v > u32::MAX as u64 {
            return None;
        }
    }
    Some(v as u32 as i32)
}

/// `'A'` or `'\n'` at the start of `s`: (value, length in characters).
fn char_literal(s: &[char]) -> Option<(i32, usize)> {
    match s {
        ['\'', '\\', e, '\'', ..] => {
            let v = match e {
                'n' => b'\n',
                't' => b'\t',
                '0' => 0,
                '\\' => b'\\',
                '\'' => b'\'',
                _ => return None,
            };
            Some((v as i32, 4))
        }
        ['\'', c, '\'', ..] if c.is_ascii() && *c != '\\' && *c != '\'' && !c.is_control() => {
            Some((*c as i32, 3))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        tokenize(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(parse_int("42"), Some(42));
        assert_eq!(parse_int("0x2A"), Some(42));
        assert_eq!(parse_int("0b101"), Some(5));
        assert_eq!(parse_int("1_000"), Some(1000));
        assert_eq!(parse_int("4294967295"), Some(-1));
        assert_eq!(parse_int("2147483648"), Some(i32::MIN));
        assert_eq!(parse_int("4294967296"), None);
        assert_eq!(parse_int("12ab"), None);
        assert_eq!(parse_int("0x"), None);
    }

    #[test]
    fn tokens_and_positions() {
        let t = tokenize("var x = 'A'; // hi\n  x <= 0x10").unwrap();
        assert_eq!(t[0].tok, Tok::Sym("var"));
        assert_eq!(t[3].tok, Tok::Int(65));
        assert_eq!(t[5].pos, Pos { line: 2, col: 3 });
        assert_eq!(t[6].tok, Tok::Sym("<="));
        assert_eq!(kinds("a<<b").len(), 4);
        assert_eq!(kinds("'\\n'")[0], Tok::Int(10));
    }

    #[test]
    fn errors() {
        assert!(tokenize("x @ y").is_err());
        assert!(tokenize("'ab'").is_err());
        assert!(tokenize("99999999999").is_err());
        assert!(tokenize("ä").is_err());
    }
}
