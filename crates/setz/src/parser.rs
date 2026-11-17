//! Recursive-descent parser (grammar in `docs/SETZWERK.md`).

use crate::ast::{BinOp, Expr, Function, Global, Program, Stmt, UnOp};
use crate::lexer::{Tok, Token};
use crate::{Error, Pos};

/// Maximum nesting of expressions and blocks; keeps the recursion of every stage bounded.
pub const MAX_DEPTH: u32 = 200;

/// Binary precedence levels from loosest to tightest.
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
                    format!(
                        "expected 'fn' or 'var' at the top level, found {}",
                        describe(&t.tok)
                    ),
                ));
            }
        }
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
                format!("expected '{s}', found {}", describe(&t.tok)),
            ))
        }
    }

    fn ident(&mut self, what: &str) -> Result<(String, Pos), Error> {
        let t = self.next();
        match t.tok {
            Tok::Ident(s) => Ok((s, t.pos)),
            other => Err(Error::new(
                t.pos,
                format!("expected {what}, found {}", describe(&other)),
            )),
        }
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(Error::new(
                self.peek().pos,
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
        let (name, _) = self.ident("a variable name")?;
        let mut init = 0i32;
        if self.eat("=") {
            let neg = self.eat("-");
            let t = self.next();
            match t.tok {
                Tok::Int(v) => init = if neg { v.wrapping_neg() } else { v },
                other => {
                    return Err(Error::new(
                        t.pos,
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
        let (name, _) = self.ident("a function name")?;
        self.expect("(")?;
        let mut params = Vec::new();
        if !self.is(")") {
            loop {
                params.push(self.ident("a parameter name")?.0);
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
                let (name, _) = self.ident("a variable name")?;
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

    fn binary(&mut self, level: usize) -> Result<Expr, Error> {
        let Some(ops) = LEVELS.get(level) else {
            return self.unary();
        };
        let mut lhs = self.binary(level + 1)?;
        // Every operator in a chain makes the (left-deep) tree one level deeper, so it counts
        // towards the nesting limit like a parenthesis.
        let mut chain = 0;
        let result = loop {
            let t = self.peek().clone();
            let op = match &t.tok {
                Tok::Sym(s) => ops.iter().find(|(text, _)| text == s).map(|(_, op)| *op),
                _ => None,
            };
            let Some(op) = op else { break Ok(lhs) };
            self.i += 1;
            chain += 1;
            if let Err(e) = self.enter() {
                break Err(e);
            }
            let rhs = match self.binary(level + 1) {
                Ok(r) => r,
                Err(e) => break Err(e),
            };
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs), t.pos);
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
                format!("expected an expression, found {}", describe(&other)),
            )),
        }
    }
}
