//! Syntax tree.

use crate::Pos;

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    And,
    Xor,
    Or,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    LogAnd,
    LogOr,
}

impl BinOp {
    /// Source spelling.
    pub fn text(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::And => "&",
            BinOp::Xor => "^",
            BinOp::Or => "|",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::LogAnd => "&&",
            BinOp::LogOr => "||",
        }
    }

    /// `true` for the six comparisons.
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
        )
    }
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

/// Expressions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Int(i32),
    Var(String, Pos),
    Call(String, Vec<Expr>, Pos),
    Read,
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>, Pos),
}

/// Statements; `line` is the source line for the comment in the assembly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    Var(String, Option<Expr>, Pos),
    Assign(String, Expr, Pos),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>, Pos),
    While(Expr, Vec<Stmt>, Pos),
    Break(Pos),
    Continue(Pos),
    Return(Option<Expr>, Pos),
    Print(Expr, Pos),
    Putc(Expr, Pos),
    Block(Vec<Stmt>),
    Expr(Expr, Pos),
}

/// A function definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

/// A global variable with its constant initial value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Global {
    pub name: String,
    pub init: i32,
    pub pos: Pos,
}

/// A whole program.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub globals: Vec<Global>,
    pub functions: Vec<Function>,
}

impl Program {
    /// The function called `name`.
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }
}
