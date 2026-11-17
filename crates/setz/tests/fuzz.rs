//! Robustness: 10 000 random inputs (byte strings, token soups, mutations of valid programs and
//! pathological nesting). The compiler must never panic, and whatever it accepts must assemble.

use stapelwerk_core::asm::{MnemonicFilter, assemble};
use stapelwerk_setz::random::{self, Rng};
use stapelwerk_setz::{compile, parse};

const VOCAB: &[&str] = &[
    "fn",
    "var",
    "if",
    "else",
    "while",
    "break",
    "continue",
    "return",
    "read",
    "print",
    "putc",
    "main",
    "x",
    "y",
    "f",
    "add",
    "r0",
    "SCREEN",
    "__x",
    "(",
    ")",
    "{",
    "}",
    ",",
    ";",
    "=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "<<",
    ">>",
    "<",
    "<=",
    ">",
    ">=",
    "==",
    "!=",
    "&",
    "^",
    "|",
    "&&",
    "||",
    "!",
    "~",
    "0",
    "1",
    "-1",
    "65535",
    "4294967295",
    "4294967296",
    "0x",
    "'a'",
    "'\\n'",
    "''",
    "//",
    "\n",
    " ",
    "@",
    "ä",
    "\u{0}",
    "\r",
];

fn check(src: &str) -> bool {
    match compile(src, "fuzz.sz") {
        Ok(asm) => {
            if let Err(d) = assemble(&asm, None, &MnemonicFilter::All) {
                panic!(
                    "accepted program does not assemble: {d:?}\n--- source\n{src}\n--- asm\n{asm}"
                );
            }
            // The interpreter must not panic on accepted programs either.
            if let Ok(p) = parse(src) {
                let _ = stapelwerk_setz::interp::run(
                    &p,
                    &[1, 2, 3],
                    stapelwerk_setz::interp::Limits {
                        steps: 20_000,
                        depth: 50,
                    },
                );
            }
            true
        }
        Err(e) => {
            assert!(!e.msg.is_empty());
            assert!(e.pos.line >= 1 && e.pos.col >= 1, "{e:?}");
            false
        }
    }
}

fn mutate(r: &mut Rng, src: &str) -> String {
    let mut chars: Vec<char> = src.chars().collect();
    for _ in 0..=r.below(4) {
        if chars.is_empty() {
            break;
        }
        let i = r.below(chars.len() as u64) as usize;
        let len = (r.below(8) as usize + 1).min(chars.len() - i);
        match r.below(4) {
            0 => {
                chars.drain(i..i + len);
            }
            1 => {
                let piece: Vec<char> = chars[i..i + len].to_vec();
                let j = r.below(chars.len() as u64) as usize;
                chars.splice(j..j, piece);
            }
            2 => {
                let tok = r.pick(VOCAB);
                chars.splice(i..i, tok.chars());
            }
            _ => chars[i] = *r.pick(&['(', ')', '{', '}', ';', '0', 'x', '-', '=', ' ']),
        }
    }
    chars.into_iter().collect()
}

#[test]
fn ten_thousand_inputs_without_panic() {
    let mut r = Rng::new(0x5E72);
    let mut accepted = 0;
    let mut total = 0;
    // 3000 random byte strings (lossily decoded).
    for _ in 0..3000 {
        let len = r.below(120) as usize;
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                if r.chance(70) {
                    *r.pick(b"fnvar(){};=+-*/%<>!&|^~ \n0123456789xyz'_")
                } else {
                    r.next_u64() as u8
                }
            })
            .collect();
        accepted += check(&String::from_utf8_lossy(&bytes)) as u32;
        total += 1;
    }
    // 3000 token soups.
    for _ in 0..3000 {
        let n = r.below(60) as usize;
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(r.pick(VOCAB));
            s.push(' ');
        }
        accepted += check(&s) as u32;
        total += 1;
    }
    // 3980 mutations of valid programs.
    for i in 0..3980 {
        let g = random::program(10_000 + i / 4);
        accepted += check(&mutate(&mut r, &g.source)) as u32;
        total += 1;
    }
    // 20 pathological inputs.
    for n in [10usize, 63, 64, 65, 5000] {
        check(&format!(
            "fn main() {{ print({}1{}); }}",
            "(".repeat(n),
            ")".repeat(n)
        ));
        check(&format!("fn main() {}{}", "{".repeat(n), "}".repeat(n)));
        check(&format!("fn main() {{ print({}1); }}", "-!~".repeat(n)));
        check(&format!("fn main() {{ print(1{}); }}", "+1".repeat(n)));
        total += 4;
    }
    assert_eq!(total, 10_000);
    eprintln!("{accepted} of {total} inputs accepted");
    assert!(
        accepted >= 200,
        "mutations should still produce valid programs ({accepted})"
    );
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let deep = format!(
        "fn main() {{ print({}1{}); }}",
        "(".repeat(5000),
        ")".repeat(5000)
    );
    let e = compile(&deep, "x").unwrap_err();
    assert!(e.msg.contains("nested too deeply"), "{e}");
    let long = format!("fn main() {{ print(1{}); }}", "+1".repeat(5000));
    assert!(compile(&long, "x").is_err());
    assert!(
        compile(
            &format!("fn main() {{ print(1{}); }}", "+1".repeat(40)),
            "x"
        )
        .is_ok()
    );
}
