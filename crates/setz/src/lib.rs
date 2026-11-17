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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub pos: Pos,
    pub msg: String,
}

impl Error {
    pub fn new(pos: Pos, msg: String) -> Self {
        Error { pos, msg }
    }
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
    let prog = parse(src)?;
    codegen::generate(&prog, src, name)
}
