//! Random valid Setzwerk programs for the differential test.
//!
//! Every generated program terminates: loops count a dedicated counter up to a small constant
//! (incremented first, so `continue` cannot skip it), helper functions only call functions
//! defined before them, and the one recursive function recurses on `n & 7`.
//! Division by zero and running out of input can happen on purpose; both sides must then
//! report the same fault after the same output.

/// SplitMix64, deterministic from a seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in 0..n (n > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

const EDGE: &[i64] = &[
    0,
    1,
    -1,
    2,
    3,
    7,
    31,
    32,
    255,
    32767,
    32768,
    -32768,
    -32769,
    65535,
    65536,
    2147483647,
    -2147483648,
    100000,
    -7,
];

const BINOPS: &[&str] = &[
    "+", "-", "*", "/", "%", "<<", ">>", "&", "^", "|", "<", "<=", ">", ">=", "==", "!=", "&&",
    "||",
];

struct FnSig {
    name: String,
    arity: usize,
}

struct Ctx {
    /// Visible variables per scope, and whether each may be assigned.
    scopes: Vec<Vec<(String, bool)>>,
    callable: Vec<(String, usize)>,
    counter: u32,
    in_loop: bool,
    reads: u32,
}

impl Ctx {
    fn vars(&self) -> Vec<&(String, bool)> {
        self.scopes.iter().flatten().collect()
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}{}", self.counter)
    }
}

/// A random program and an input that is usually long enough.
pub struct Generated {
    pub source: String,
    pub input: Vec<i32>,
}

/// Generates one program from `seed`.
pub fn program(seed: u64) -> Generated {
    let mut r = Rng::new(seed);
    let mut src = String::new();
    let mut globals = Vec::new();
    for i in 0..r.below(3) {
        let name = format!("g{i}");
        src.push_str(&format!("var {name} = {};\n", literal(&mut r)));
        globals.push(name);
    }
    let mut fns: Vec<FnSig> = Vec::new();
    // The recursive function, if any, comes first so that later functions may call it.
    if r.chance(50) {
        let extra = r.below(3) as usize;
        let mut params = vec!["n".to_string()];
        params.extend((0..extra).map(|i| format!("a{i}")));
        let mut ctx = new_ctx(&globals, &params, &[]);
        let base = expr(&mut r, &mut ctx, 2);
        let step = expr(&mut r, &mut ctx, 2);
        let rec_args: Vec<String> = (0..extra).map(|_| expr(&mut r, &mut ctx, 2)).collect();
        let mut call_args = vec!["n - 1".to_string()];
        call_args.extend(rec_args);
        let op = r.pick(&["+", "-", "^", "*"]);
        src.push_str(&format!(
            "fn rec({}) {{\n  if (n <= 0) {{ return {base}; }}\n  var t = rec({});\n  return t {op} ({step});\n}}\n",
            params.join(", "),
            call_args.join(", ")
        ));
        fns.push(FnSig {
            name: "rec".to_string(),
            arity: params.len(),
        });
    }
    for i in 0..r.below(4) {
        let arity = *r.pick(&[0usize, 1, 2, 3, 4, 5, 6]);
        let params: Vec<String> = (0..arity).map(|k| format!("p{k}")).collect();
        let mut ctx = new_ctx(&globals, &params, &fns);
        let body = stmts(&mut r, &mut ctx, 2, 4);
        let ret = expr(&mut r, &mut ctx, 3);
        src.push_str(&format!(
            "fn f{i}({}) {{\n{body}  return {ret};\n}}\n",
            params.join(", ")
        ));
        fns.push(FnSig {
            name: format!("f{i}"),
            arity,
        });
    }
    let mut ctx = new_ctx(&globals, &[], &fns);
    let body = stmts(&mut r, &mut ctx, 3, 10);
    src.push_str(&format!("fn main() {{\n{body}"));
    for g in &globals {
        src.push_str(&format!("  print({g});\n"));
    }
    src.push_str("}\n");
    // Usually enough input; sometimes deliberately too little.
    let n = if r.chance(85) {
        ctx.reads as u64 * 8 + 4
    } else {
        r.below(4)
    };
    let input = (0..n).map(|_| literal(&mut r) as i32).collect();
    Generated { source: src, input }
}

fn new_ctx(globals: &[String], params: &[String], fns: &[FnSig]) -> Ctx {
    let mut scopes = vec![
        globals
            .iter()
            .map(|g| (g.clone(), true))
            .collect::<Vec<_>>(),
    ];
    scopes.push(params.iter().map(|p| (p.clone(), true)).collect());
    Ctx {
        scopes,
        callable: fns.iter().map(|f| (f.name.clone(), f.arity)).collect(),
        counter: 0,
        in_loop: false,
        reads: 0,
    }
}

fn literal(r: &mut Rng) -> i64 {
    match r.below(3) {
        0 => *r.pick(EDGE),
        1 => r.below(20) as i64 - 5,
        _ => (r.next_u64() as u32 as i32) as i64,
    }
}

fn expr(r: &mut Rng, ctx: &mut Ctx, depth: u32) -> String {
    let leaf = depth == 0 || r.chance(30);
    if leaf {
        let vars = ctx.vars();
        return match r.below(10) {
            0..=4 if !vars.is_empty() => r.pick(&vars).0.clone(),
            5 => {
                ctx.reads += 1;
                "read()".to_string()
            }
            _ => {
                let v = literal(r);
                if v < 0 {
                    format!("({v})")
                } else {
                    v.to_string()
                }
            }
        };
    }
    match r.below(10) {
        0 => {
            let op = r.pick(&["-", "!", "~"]);
            format!("{op}{}", expr(r, ctx, depth - 1))
        }
        1 if !ctx.callable.is_empty() => {
            let (name, arity) = r.pick(&ctx.callable).clone();
            let args: Vec<String> = (0..arity)
                .map(|i| {
                    let a = expr(r, ctx, depth - 1);
                    // Keep the recursion depth of `rec` at most 7.
                    if name == "rec" && i == 0 {
                        format!("({a}) & 7")
                    } else {
                        a
                    }
                })
                .collect();
            format!("{name}({})", args.join(", "))
        }
        _ => {
            let op = *r.pick(BINOPS);
            let a = expr(r, ctx, depth - 1);
            let mut b = expr(r, ctx, depth - 1);
            if (op == "/" || op == "%") && r.chance(80) {
                b = format!("({b} | 1)");
            }
            format!("({a} {op} {b})")
        }
    }
}

fn stmts(r: &mut Rng, ctx: &mut Ctx, depth: u32, max: u64) -> String {
    let mut out = String::new();
    for _ in 0..=r.below(max) {
        out.push_str(&stmt(r, ctx, depth));
    }
    out
}

fn stmt(r: &mut Rng, ctx: &mut Ctx, depth: u32) -> String {
    let kind = if depth == 0 { r.below(5) } else { r.below(9) };
    match kind {
        0 | 1 => {
            let name = ctx.fresh("v");
            let e = expr(r, ctx, 3);
            if let Some(s) = ctx.scopes.last_mut() {
                s.push((name.clone(), true));
            }
            format!("var {name} = {e};\n")
        }
        2 => {
            let targets: Vec<String> = ctx
                .vars()
                .into_iter()
                .filter(|(_, w)| *w)
                .map(|(n, _)| n.clone())
                .collect();
            if targets.is_empty() {
                format!("print({});\n", expr(r, ctx, 3))
            } else {
                let t = r.pick(&targets).clone();
                format!("{t} = {};\n", expr(r, ctx, 3))
            }
        }
        3 => format!("print({});\n", expr(r, ctx, 3)),
        4 => {
            if ctx.in_loop && r.chance(30) {
                if r.chance(50) {
                    "break;\n".to_string()
                } else {
                    "continue;\n".to_string()
                }
            } else {
                format!("putc(65 + (({}) & 15));\n", expr(r, ctx, 2))
            }
        }
        5 | 6 => {
            let c = expr(r, ctx, 3);
            let then = scoped(r, ctx, depth - 1);
            if r.chance(50) {
                let els = scoped(r, ctx, depth - 1);
                format!("if ({c}) {{\n{then}}} else {{\n{els}}}\n")
            } else {
                format!("if ({c}) {{\n{then}}}\n")
            }
        }
        7 => {
            let i = ctx.fresh("i");
            let k = r.below(5);
            let extra = if r.chance(30) {
                format!(" && {}", expr(r, ctx, 2))
            } else {
                String::new()
            };
            // The counter is visible but read-only for the generator.
            ctx.scopes.push(vec![(i.clone(), false)]);
            let was = ctx.in_loop;
            ctx.in_loop = true;
            let body = stmts(r, ctx, depth - 1, 3);
            ctx.in_loop = was;
            ctx.scopes.pop();
            format!("{{\nvar {i} = 0;\nwhile ({i} < {k}{extra}) {{\n{i} = {i} + 1;\n{body}}}\n}}\n")
        }
        _ => {
            let e = expr(r, ctx, 3);
            format!("{e};\n")
        }
    }
}

fn scoped(r: &mut Rng, ctx: &mut Ctx, depth: u32) -> String {
    ctx.scopes.push(Vec::new());
    let s = stmts(r, ctx, depth, 3);
    ctx.scopes.pop();
    s
}
