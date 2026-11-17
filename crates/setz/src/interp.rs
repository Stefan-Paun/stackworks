//! Tree-walking interpreter: the reference semantics of `docs/SETZWERK.md`, written
//! independently of the code generator. The differential test compares both.
//!
//! It expects a program that the compiler accepted (names and arities are checked there).

use crate::ast::{BinOp, Expr, Program, Stmt, UnOp};
use stapelwerk_core::consts::{OUT_KIND_BYTE, OUT_KIND_INT, OUTPUT_LIMIT};

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `main` returned.
    Halted,
    /// The machine would stop with this fault key (`fault.div_zero`, `fault.input_end`,
    /// `fault.output_limit`).
    Fault(&'static str),
    /// The interpreter's own step or depth limit was hit; no comparison possible.
    Limit,
    /// The program has no `main` or is not one the compiler accepts.
    Invalid,
}

/// Output tape (kind, value) as in the machine, and the outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunResult {
    pub out: Vec<(u8, i32)>,
    pub outcome: Outcome,
}

/// Limits of one run.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub steps: u64,
    pub depth: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            steps: 2_000_000,
            depth: 100,
        }
    }
}

enum Stop {
    Fault(&'static str),
    Limit,
    Invalid,
}

enum Flow {
    Normal,
    Break,
    Continue,
    Return(i32),
}

struct Interp<'a> {
    prog: &'a Program,
    input: &'a [i32],
    next_in: usize,
    globals: Vec<(String, i32)>,
    /// Scopes of the current call (innermost last).
    scopes: Vec<Vec<(String, i32)>>,
    out: Vec<(u8, i32)>,
    steps: u64,
    depth: u32,
    limits: Limits,
}

/// Runs `main` with the given number input.
pub fn run(prog: &Program, input: &[i32], limits: Limits) -> RunResult {
    let mut it = Interp {
        prog,
        input,
        next_in: 0,
        globals: prog
            .globals
            .iter()
            .map(|g| (g.name.clone(), g.init))
            .collect(),
        scopes: Vec::new(),
        out: Vec::new(),
        steps: 0,
        depth: 0,
        limits,
    };
    let outcome = match it.call("main", Vec::new()) {
        Ok(_) => Outcome::Halted,
        Err(Stop::Fault(k)) => Outcome::Fault(k),
        Err(Stop::Limit) => Outcome::Limit,
        Err(Stop::Invalid) => Outcome::Invalid,
    };
    RunResult {
        out: it.out,
        outcome,
    }
}

impl Interp<'_> {
    fn tick(&mut self) -> Result<(), Stop> {
        self.steps += 1;
        if self.steps > self.limits.steps {
            Err(Stop::Limit)
        } else {
            Ok(())
        }
    }

    fn call(&mut self, name: &str, args: Vec<i32>) -> Result<i32, Stop> {
        let f = self.prog.function(name).ok_or(Stop::Invalid)?;
        if f.params.len() != args.len() {
            return Err(Stop::Invalid);
        }
        if self.depth >= self.limits.depth {
            return Err(Stop::Limit);
        }
        self.depth += 1;
        let frame: Vec<(String, i32)> = f.params.iter().cloned().zip(args).collect();
        let saved = std::mem::replace(&mut self.scopes, vec![frame]);
        let r = self.block(&f.body);
        self.scopes = saved;
        self.depth -= 1;
        match r? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(0),
            Flow::Break | Flow::Continue => Err(Stop::Invalid),
        }
    }

    fn slot(&mut self, name: &str) -> Result<&mut i32, Stop> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some((_, v)) = scope.iter_mut().rev().find(|(n, _)| n == name) {
                return Ok(v);
            }
        }
        self.globals
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
            .ok_or(Stop::Invalid)
    }

    fn block(&mut self, stmts: &[Stmt]) -> Result<Flow, Stop> {
        self.scopes.push(Vec::new());
        let mut flow = Flow::Normal;
        for s in stmts {
            match self.stmt(s) {
                Ok(Flow::Normal) => {}
                Ok(other) => {
                    flow = other;
                    break;
                }
                Err(e) => {
                    self.scopes.pop();
                    return Err(e);
                }
            }
        }
        self.scopes.pop();
        Ok(flow)
    }

    fn stmt(&mut self, s: &Stmt) -> Result<Flow, Stop> {
        self.tick()?;
        match s {
            Stmt::Var(name, init, _) => {
                let v = match init {
                    Some(e) => self.expr(e)?,
                    None => 0,
                };
                self.scopes
                    .last_mut()
                    .ok_or(Stop::Invalid)?
                    .push((name.clone(), v));
            }
            Stmt::Assign(name, e, _) => {
                let v = self.expr(e)?;
                *self.slot(name)? = v;
            }
            Stmt::If(c, then, els, _) => {
                if self.expr(c)? != 0 {
                    return self.block(then);
                } else if let Some(els) = els {
                    return self.block(els);
                }
            }
            Stmt::While(c, body, _) => loop {
                self.tick()?;
                if self.expr(c)? == 0 {
                    break;
                }
                match self.block(body)? {
                    Flow::Break => break,
                    Flow::Normal | Flow::Continue => {}
                    ret @ Flow::Return(_) => return Ok(ret),
                }
            },
            Stmt::Break(_) => return Ok(Flow::Break),
            Stmt::Continue(_) => return Ok(Flow::Continue),
            Stmt::Return(e, _) => {
                let v = match e {
                    Some(e) => self.expr(e)?,
                    None => 0,
                };
                return Ok(Flow::Return(v));
            }
            Stmt::Print(e, _) => {
                let v = self.expr(e)?;
                self.emit(OUT_KIND_INT, v)?;
            }
            Stmt::Putc(e, _) => {
                let v = self.expr(e)?;
                self.emit(OUT_KIND_BYTE, v & 0xFF)?;
            }
            Stmt::Block(b) => return self.block(b),
            Stmt::Expr(e, _) => {
                self.expr(e)?;
            }
        }
        Ok(Flow::Normal)
    }

    fn emit(&mut self, kind: u8, v: i32) -> Result<(), Stop> {
        if self.out.len() >= OUTPUT_LIMIT as usize {
            return Err(Stop::Fault("fault.output_limit"));
        }
        self.out.push((kind, v));
        Ok(())
    }

    fn expr(&mut self, e: &Expr) -> Result<i32, Stop> {
        Ok(match e {
            Expr::Int(v) => *v,
            Expr::Var(name, _) => *self.slot(name)?,
            Expr::Read => {
                let v = *self
                    .input
                    .get(self.next_in)
                    .ok_or(Stop::Fault("fault.input_end"))?;
                self.next_in += 1;
                v
            }
            Expr::Call(name, args, _) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.expr(a)?);
                }
                self.tick()?;
                self.call(name, vals)?
            }
            Expr::Unary(op, inner) => {
                let v = self.expr(inner)?;
                match op {
                    UnOp::Neg => v.wrapping_neg(),
                    UnOp::BitNot => !v,
                    UnOp::Not => (v == 0) as i32,
                }
            }
            Expr::Binary(BinOp::LogAnd, l, r, _) => {
                (self.expr(l)? != 0 && self.expr(r)? != 0) as i32
            }
            Expr::Binary(BinOp::LogOr, l, r, _) => {
                (self.expr(l)? != 0 || self.expr(r)? != 0) as i32
            }
            Expr::Binary(op, l, r, _) => {
                let a = self.expr(l)?;
                let b = self.expr(r)?;
                match op {
                    BinOp::Add => a.wrapping_add(b),
                    BinOp::Sub => a.wrapping_sub(b),
                    BinOp::Mul => a.wrapping_mul(b),
                    BinOp::Div | BinOp::Rem if b == 0 => return Err(Stop::Fault("fault.div_zero")),
                    BinOp::Div => a.wrapping_div(b),
                    BinOp::Rem => a.wrapping_rem(b),
                    BinOp::Shl => a.wrapping_shl(b as u32 & 31),
                    BinOp::Shr => a.wrapping_shr(b as u32 & 31),
                    BinOp::And => a & b,
                    BinOp::Xor => a ^ b,
                    BinOp::Or => a | b,
                    BinOp::Lt => (a < b) as i32,
                    BinOp::Le => (a <= b) as i32,
                    BinOp::Gt => (a > b) as i32,
                    BinOp::Ge => (a >= b) as i32,
                    BinOp::Eq => (a == b) as i32,
                    BinOp::Ne => (a != b) as i32,
                    BinOp::LogAnd | BinOp::LogOr => return Err(Stop::Invalid),
                }
            }
        })
    }
}
