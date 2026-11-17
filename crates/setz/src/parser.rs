//! Recursive-descent parser (grammar in `docs/SETZWERK.md`).

use crate::ast::{BinOp, Expr, Function, Global, Program, Stmt, UnOp};
use crate::lexer::{Tok, Token};
use crate::{Error, Pos};

/// Maximum nesting of expressions and blocks; keeps the recursion of every stage bounded.
pub const MAX_DEPTH: u32 = 64;

/// Binary precedence levels from loosest to tightest (all left-associative).
const LEVELS: &[&[(&str, BinOp)]] = &[
    &[("||", BinOp::LogOr)],
    &[("&&", BinOp::LogAnd)],
    &[("|", BinOp::Or)],
    &[("^", BinOp::Xor)],
    &[("&", BinOp::And)],
    &[("==", BinOp::Eq), ("!=", BinOp::Ne)],
    &[
        ("<", BinOp::Lt),
        ("<=", BinOp::Le),
        (">", BinOp::Gt),
        (">=", BinOp::Ge),
    ],
    &[("<<", BinOp::Shl), (">>", BinOp::Shr)],
    &[("+", BinOp::Add), ("-", BinOp::Sub)],
    &[("*", BinOp::Mul), ("/", BinOp::Div), ("%", BinOp::Rem)],
];

struct Parser {
    toks: Vec<Token>,
    i: usize,
    depth: u32,
}

/// Parses tokens (ending with [`Tok::Eof`]) into a program.
pub fn parse(toks: Vec<Token>) -> Result<Program, Error> {
    let mut p = Parser {
        toks,
        i: 0,
        depth: 0,
    };
    let mut prog = Program::default();
    loop {
        let t = p.peek().clone();
        match t.tok {
            Tok::Eof => return Ok(prog),
            Tok::Sym("var") => prog.globals.push(p.global()?),
            Tok::Sym("fn") => prog.functions.push(p.function()?),
            _ => {
                return Err(Error::new(
                    t.pos,
                    "setz.expected_top",
                    vec![("found", found(&t.tok))],
                    format!(
                        "expected 'fn' or 'var' at the top level, found {}",
                        describe(&t.tok)
                    ),
                ));
            }
        }
    }
}

/// The found token as an error argument: the number or symbol text, empty at the end of file.
fn found(t: &Tok) -> String {
    match t {
        Tok::Int(v) => v.to_string(),
        Tok::Ident(s) => s.clone(),
        Tok::Sym(s) => s.to_string(),
        Tok::Eof => String::new(),
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Int(v) => format!("number {v}"),
        Tok::Ident(s) => format!("'{s}'"),
        Tok::Sym(s) => format!("'{s}'"),
        Tok::Eof => "end of file".to_string(),
    }
}

impl Parser {
    fn peek(&self) -> &Token {
        // The token list always ends with Eof and `i` never moves past it.
        &self.toks[self.i.min(self.toks.len() - 1)]
    }

    fn peek2(&self) -> &Tok {
        &self.toks[(self.i + 1).min(self.toks.len() - 1)].tok
    }

    fn next(&mut self) -> Token {
        let t = self.peek().clone();
        if t.tok != Tok::Eof {
            self.i += 1;
        }
        t
    }

    fn is(&self, s: &str) -> bool {
        matches!(&self.peek().tok, Tok::Sym(x) if *x == s)
    }

    fn eat(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, s: &str) -> Result<Pos, Error> {
        let t = self.peek().clone();
        if self.eat(s) {
            Ok(t.pos)
        } else {
            Err(Error::new(
                t.pos,
                "setz.expected_token",
                vec![("token", s.to_string()), ("found", found(&t.tok))],
                format!("expected '{s}', found {}", describe(&t.tok)),
            ))
        }
    }

    /// Reads a name; `what` is `var`, `fn` or `param` (error argument).
    fn ident(&mut self, what: &'static str) -> Result<(String, Pos), Error> {
        let t = self.next();
        match t.tok {
            Tok::Ident(s) => Ok((s, t.pos)),
            other => {
                let english = match what {
                    "fn" => "a function name",
                    "param" => "a parameter name",
                    _ => "a variable name",
                };
                Err(Error::new(
                    t.pos,
                    "setz.expected_name",
                    vec![("what", what.to_string()), ("found", found(&other))],
                    format!("expected {english}, found {}", describe(&other)),
                ))
            }
        }
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(Error::new(
                self.peek().pos,
                "setz.too_deep",
                vec![("max", MAX_DEPTH.to_string())],
                format!("nested too deeply (more than {MAX_DEPTH} levels)"),
            ))
        } else {
            Ok(())
        }
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn global(&mut self) -> Result<Global, Error> {
        let pos = self.expect("var")?;
        let (name, _) = self.ident("var")?;
        let mut init = 0i32;
        if self.eat("=") {
            let neg = self.eat("-");
            let t = self.next();
            match t.tok {
                Tok::Int(v) => init = if neg { v.wrapping_neg() } else { v },
                other => {
                    return Err(Error::new(
                        t.pos,
                        "setz.global_not_constant",
                        vec![("found", found(&other))],
                        format!(
                            "a global variable needs a constant number, found {}",
                            describe(&other)
                        ),
                    ));
                }
            }
        }
        self.expect(";")?;
        Ok(Global { name, init, pos })
    }

    fn function(&mut self) -> Result<Function, Error> {
        let pos = self.expect("fn")?;
        let (name, _) = self.ident("fn")?;
        self.expect("(")?;
        let mut params = Vec::new();
        if !self.is(")") {
            loop {
                params.push(self.ident("param")?.0);
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.expect(")")?;
        let body = self.block()?;
        Ok(Function {
            name,
            params,
            body,
            pos,
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, Error> {
        self.expect("{")?;
        self.enter()?;
        let mut stmts = Vec::new();
        while !self.is("}") {
            if self.peek().tok == Tok::Eof {
                let pos = self.peek().pos;
                return Err(Error::new(
                    pos,
                    "setz.missing_brace",
                    Vec::new(),
                    "missing '}' before end of file".to_string(),
                ));
            }
            stmts.push(self.statement()?);
        }
        self.i += 1;
        self.leave();
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt, Error> {
        let t = self.peek().clone();
        let pos = t.pos;
        let stmt = match t.tok {
            Tok::Sym("var") => {
                self.i += 1;
                let (name, _) = self.ident("var")?;
                let init = if self.eat("=") {
                    Some(self.expr()?)
                } else {
                    None
                };
                self.expect(";")?;
                Stmt::Var(name, init, pos)
            }
            Tok::Ident(ref name) if *self.peek2() == Tok::Sym("=") => {
                let name = name.clone();
                self.i += 2;
                let e = self.expr()?;
                self.expect(";")?;
                Stmt::Assign(name, e, pos)
            }
            Tok::Sym("if") => self.if_stmt()?,
            Tok::Sym("while") => {
                self.i += 1;
                self.expect("(")?;
                let c = self.expr()?;
                self.expect(")")?;
                let body = self.block()?;
                Stmt::While(c, body, pos)
            }
            Tok::Sym("break") => {
                self.i += 1;
                self.expect(";")?;
                Stmt::Break(pos)
            }
            Tok::Sym("continue") => {
                self.i += 1;
                self.expect(";")?;
                Stmt::Continue(pos)
            }
            Tok::Sym("return") => {
                self.i += 1;
                let e = if self.is(";") {
                    None
                } else {
                    Some(self.expr()?)
                };
                self.expect(";")?;
                Stmt::Return(e, pos)
            }
            Tok::Sym(kw @ ("print" | "putc")) => {
                self.i += 1;
                self.expect("(")?;
                let e = self.expr()?;
                self.expect(")")?;
                self.expect(";")?;
                if kw == "print" {
                    Stmt::Print(e, pos)
                } else {
                    Stmt::Putc(e, pos)
                }
            }
            Tok::Sym("{") => Stmt::Block(self.block()?),
            _ => {
                let e = self.expr()?;
                self.expect(";")?;
                Stmt::Expr(e, pos)
            }
        };
        Ok(stmt)
    }

    fn if_stmt(&mut self) -> Result<Stmt, Error> {
        let pos = self.expect("if")?;
        self.enter()?;
        self.expect("(")?;
        let c = self.expr()?;
        self.expect(")")?;
        let then = self.block()?;
        let els = if self.eat("else") {
            if self.is("if") {
                Some(vec![self.if_stmt()?])
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        self.leave();
        Ok(Stmt::If(c, then, els, pos))
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        self.enter()?;
        let e = self.binary(0);
        self.leave();
        e
    }

    /// The operator at the current token, if it binds at `min_level` or tighter.
    fn binary_op(&self, min_level: usize) -> Option<(usize, BinOp)> {
        let Tok::Sym(s) = &self.peek().tok else {
            return None;
        };
        LEVELS
            .iter()
            .enumerate()
            .skip(min_level)
            .find_map(|(lvl, ops)| {
                ops.iter()
                    .find(|(text, _)| text == s)
                    .map(|(_, op)| (lvl, *op))
            })
    }

    /// Precedence climbing: parses operators that bind at `min_level` or tighter. Recursion only
    /// happens for a tighter right operand, so a chain such as `a + b + c` stays flat here.
    fn binary(&mut self, min_level: usize) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;
        // Every operator makes the (left-deep) tree one level deeper, so it counts towards the
        // nesting limit like a parenthesis.
        let mut chain = 0;
        let result = loop {
            let Some((level, op)) = self.binary_op(min_level) else {
                break Ok(lhs);
            };
            let pos = self.next().pos;
            chain += 1;
            if let Err(e) = self.enter() {
                break Err(e);
            }
            let rhs = match self.binary(level + 1) {
                Ok(r) => r,
                Err(e) => break Err(e),
            };
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs), pos);
        };
        for _ in 0..chain {
            self.leave();
        }
        result
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        let op = if self.eat("-") {
            UnOp::Neg
        } else if self.eat("!") {
            UnOp::Not
        } else if self.eat("~") {
            UnOp::BitNot
        } else {
            return self.primary();
        };
        self.enter()?;
        let inner = self.unary();
        self.leave();
        let inner = inner?;
        // Fold a negated literal so that `-2147483648` and `-5` are plain constants.
        Ok(match (op, inner) {
            (UnOp::Neg, Expr::Int(v)) => Expr::Int(v.wrapping_neg()),
            (op, inner) => Expr::Unary(op, Box::new(inner)),
        })
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let t = self.next();
        match t.tok {
            Tok::Int(v) => Ok(Expr::Int(v)),
            Tok::Sym("read") => {
                self.expect("(")?;
                self.expect(")")?;
                Ok(Expr::Read)
            }
            Tok::Sym("(") => {
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Ident(name) => {
                if !self.eat("(") {
                    return Ok(Expr::Var(name, t.pos));
                }
                let mut args = Vec::new();
                if !self.is(")") {
                    loop {
                        args.push(self.expr()?);
                        if !self.eat(",") {
                            break;
                        }
                    }
                }
                self.expect(")")?;
                Ok(Expr::Call(name, args, t.pos))
            }
            other => Err(Error::new(
                t.pos,
                "setz.expected_expr",
                vec![("found", found(&other))],
                format!("expected an expression, found {}", describe(&other)),
            )),
        }
    }
}
