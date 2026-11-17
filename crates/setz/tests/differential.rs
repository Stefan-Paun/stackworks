//! Differential test: 200 random programs (`stapelwerk_setz::random`) are compiled, assembled and
//! run on the machine, and also executed by the tree-walking interpreter. Output tape and end
//! state must be the same.

use stapelwerk_core::asm::{MnemonicFilter, assemble};
use stapelwerk_core::consts::MAX_BUDGET;
use stapelwerk_core::machine::{Machine, MachineConfig, Status};
use stapelwerk_setz::interp::{self, Limits, Outcome};
use stapelwerk_setz::{compile, parse, random};

/// What the machine did: output tape and end state.
fn run_machine(asm: &str, input: &[i32]) -> (Vec<(u8, i32)>, Result<(), String>) {
    let program = match assemble(asm, None, &MnemonicFilter::All) {
        Ok(p) => p,
        Err(d) => panic!("compiler output does not assemble: {d:?}\n{asm}"),
    };
    let config = MachineConfig {
        budget: MAX_BUDGET,
        ..MachineConfig::default()
    };
    let mut m = Machine::new(&program, config);
    m.tapes.in_ints = input.to_vec();
    let status = loop {
        match m.run(u64::MAX) {
            Status::Paused => continue,
            s => break s,
        }
    };
    let n = m.state.out_len as usize;
    let out = m
        .tapes
        .out_kinds
        .iter()
        .copied()
        .zip(m.tapes.out_vals.iter().copied())
        .take(n)
        .collect();
    let end = match status {
        Status::Halted => Ok(()),
        Status::Fault => Err(m
            .state
            .fault
            .as_ref()
            .map_or("fault", |f| f.key())
            .to_string()),
        other => Err(format!("status {}", other.name())),
    };
    (out, end)
}

#[test]
fn compiled_code_matches_interpreter() {
    let mut compared = 0;
    let mut skipped = 0;
    let mut halted = 0;
    let mut faults = 0;
    let mut seed = 1u64;
    while compared < 200 {
        assert!(seed < 600, "too many skipped programs ({skipped})");
        let g = random::program(seed);
        seed += 1;
        let asm = match compile(&g.source, "random.sz") {
            Ok(a) => a,
            Err(e) => panic!("generated program does not compile: {e}\n{}", g.source),
        };
        let ast = parse(&g.source).expect("parses");
        let expected = interp::run(&ast, &g.input, Limits::default());
        if matches!(expected.outcome, Outcome::Limit) {
            skipped += 1;
            continue;
        }
        let (out, end) = run_machine(&asm, &g.input);
        if end.as_ref().err().map(String::as_str) == Some("status BUDGET_EXCEEDED") {
            skipped += 1;
            continue;
        }
        let want_end = match expected.outcome {
            Outcome::Halted => Ok(()),
            Outcome::Fault(k) => Err(k.to_string()),
            other => panic!("unexpected interpreter outcome {other:?}"),
        };
        assert_eq!(
            (&out, &end),
            (&expected.out, &want_end),
            "seed {} differs\n--- source\n{}\n--- input {:?}\n--- assembly\n{asm}",
            seed - 1,
            g.source,
            g.input
        );
        if end.is_ok() {
            halted += 1;
        } else {
            faults += 1;
        }
        compared += 1;
    }
    eprintln!("compared {compared} (halted {halted}, faults {faults}), skipped {skipped}");
    assert!(halted >= 100, "only {halted} programs ran to the end");
    assert!(skipped <= 40, "{skipped} programs skipped");
}
