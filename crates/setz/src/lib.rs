//! Setzwerk: a small C-like language compiled to Stapelwerk assembly (`docs/SETZWERK.md`).
//!
//! Pipeline: [`lexer`] → [`parser`] → [`codegen`] (which also checks names and arities). The
//! [`interp`] module is an independent tree-walking interpreter used as the reference in the
//! differential test, and [`random`] generates random valid programs for that test.

#![forbid(unsafe_code)]

pub mod ast;
pub mod codegen;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod random;

use std::fmt;

/// A position in the source: 1-based line and column (columns count characters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

/// A compile error with its position.
///
/// `key` and `args` identify the error independently of the language (the web view translates
/// them, see `docs/SETZWERK.md`, "Error keys"); `msg` is the English text for the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub pos: Pos,
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
    pub msg: String,
}

impl Error {
    pub fn new(
        pos: Pos,
        key: &'static str,
        args: Vec<(&'static str, String)>,
        msg: String,
    ) -> Self {
        Error {
            pos,
            key,
            args,
            msg,
        }
    }
}

/// Every error key the compiler can produce, in a fixed order (checked against the web texts).
pub const ERROR_KEYS: [&str; 24] = [
    "setz.bad_number",
    "setz.bad_char",
    "setz.unexpected_char",
    "setz.expected_top",
    "setz.expected_token",
    "setz.expected_name",
    "setz.too_deep",
    "setz.global_not_constant",
    "setz.missing_brace",
    "setz.expected_expr",
    "setz.reserved_name",
    "setz.duplicate",
    "setz.main_params",
    "setz.too_many_params",
    "setz.duplicate_param",
    "setz.not_a_variable",
    "setz.unknown_variable",
    "setz.too_many_locals",
    "setz.already_declared",
    "setz.is_function_name",
    "setz.break_outside_loop",
    "setz.not_a_function",
    "setz.unknown_function",
    "setz.arity",
];

/// Language of the comments in the generated assembly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    De,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.pos.line, self.pos.col, self.msg)
    }
}

/// Parses `src` into a syntax tree.
pub fn parse(src: &str) -> Result<ast::Program, Error> {
    parser::parse(lexer::tokenize(src)?)
}

/// Compiles `src` to commented Stapelwerk assembly. `name` appears in the header comment.
pub fn compile(src: &str, name: &str) -> Result<String, Error> {
    compile_in(src, name, Lang::En)
}

/// Like [`compile`], with the comments of the generated assembly in `lang`. The instructions
/// are identical in every language.
pub fn compile_in(src: &str, name: &str, lang: Lang) -> Result<String, Error> {
    let prog = parse(src)?;
    codegen::generate(&prog, src, name, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `"setz.…"` key in the sources is listed in [`ERROR_KEYS`] and vice versa.
    #[test]
    fn error_keys_are_complete() {
        let sources = [
            include_str!("lexer.rs"),
            include_str!("parser.rs"),
            include_str!("codegen.rs"),
        ];
        let mut used: Vec<&str> = Vec::new();
        for s in sources {
            for part in s.split("\"setz.").skip(1) {
                let end = part.find('"').unwrap_or(0);
                used.push(&part[..end]);
            }
        }
        for k in &used {
            assert!(
                ERROR_KEYS.contains(&format!("setz.{k}").as_str()),
                "{k} missing in ERROR_KEYS"
            );
        }
        for k in ERROR_KEYS {
            assert!(used.contains(&&k[5..]), "{k} is never produced");
        }
    }

    /// German and English output differ only in comments.
    #[test]
    fn languages_differ_only_in_comments() {
        let src = "var g = 3;\nfn f(a, b, c, d, e) { return a + e; }\nfn main() {\n  var i = 0;\n  while (i < g) { print(f(i, 1, 2, 3, 4) == 5); i = i + 1; }\n}\n";
        let strip = |s: String| -> Vec<String> {
            s.lines()
                .map(|l| l.split(';').next().unwrap_or("").trim_end().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        };
        let en = compile_in(src, "t.sz", Lang::En).unwrap();
        let de = compile_in(src, "t.sz", Lang::De).unwrap();
        assert_ne!(en, de);
        assert!(de.contains("; Zeile 5:"), "{de}");
        assert_eq!(strip(en), strip(de));
    }

    #[test]
    fn errors_carry_key_and_args() {
        let e = compile("fn main() { f(1); }\nfn f(a, b) { return a; }", "t").unwrap_err();
        assert_eq!(e.key, "setz.arity");
        assert_eq!(
            e.args,
            vec![
                ("name", "f".to_string()),
                ("expected", "2".to_string()),
                ("given", "1".to_string())
            ]
        );
        let e = compile("fn main() { print(1) }", "t").unwrap_err();
        assert_eq!(e.key, "setz.expected_token");
        assert_eq!(e.args[0], ("token", ";".to_string()));
        assert_eq!(e.args[1], ("found", "}".to_string()));
    }
}
