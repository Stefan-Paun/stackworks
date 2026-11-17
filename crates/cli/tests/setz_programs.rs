//! Setzwerk end to end through the real binary: every program in `crates/setz/programs/` is
//! compiled with `stapelwerk compile` and run with `stapelwerk run`; its output must equal the
//! `.out` file next to it. Every file in `crates/setz/levels/` is compiled and must pass all tests
//! of the level with the same name.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("directory exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "sz"))
        .collect();
    files.sort();
    files
}

/// Compiles `src` into the temporary directory and returns the path of the assembly.
fn compile(src: &Path) -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(src.file_stem().unwrap())
        .with_extension("s");
    let res = Command::new(env!("CARGO_BIN_EXE_stapelwerk"))
        .arg("compile")
        .arg(src)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("run stapelwerk compile");
    assert!(
        res.status.success(),
        "compile {} failed:\n{}",
        src.display(),
        String::from_utf8_lossy(&res.stderr)
    );
    out
}

#[test]
fn example_programs_run_correctly() {
    let files = sources(&repo().join("crates/setz/programs"));
    assert!(files.len() >= 25, "only {} example programs", files.len());
    let mut failures = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).unwrap();
        let input = src
            .lines()
            .find_map(|l| l.strip_prefix("// in:"))
            .unwrap_or("")
            .trim()
            .to_string();
        let expected = std::fs::read_to_string(file.with_extension("out")).expect(".out file");
        let asm = compile(file);
        let res = Command::new(env!("CARGO_BIN_EXE_stapelwerk"))
            .arg("run")
            .arg(&asm)
            .arg("--in")
            .arg(&input)
            .output()
            .expect("run stapelwerk run");
        let got = String::from_utf8_lossy(&res.stdout);
        if !res.status.success() || got != expected {
            failures.push(format!(
                "{}: expected {expected:?}, got {got:?}\n{}",
                file.display(),
                String::from_utf8_lossy(&res.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compiled_level_solutions_pass() {
    let files = sources(&repo().join("crates/setz/levels"));
    assert!(files.len() >= 6, "only {} level solutions", files.len());
    let mut failures = Vec::new();
    for file in &files {
        let id = file.file_stem().unwrap().to_string_lossy().into_owned();
        let level = repo().join("levels").join(format!("{id}.lvl"));
        assert!(level.exists(), "no level {id}");
        let asm = compile(file);
        let res = Command::new(env!("CARGO_BIN_EXE_stapelwerk"))
            .arg("run")
            .arg(&asm)
            .arg("--level")
            .arg(&level)
            .output()
            .expect("run stapelwerk run --level");
        if !res.status.success() {
            failures.push(format!(
                "{id}:\n{}{}",
                String::from_utf8_lossy(&res.stdout),
                String::from_utf8_lossy(&res.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compile_errors_have_positions() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let bad = dir.join("bad.sz");
    std::fs::write(&bad, "fn main() {\n    print(x);\n}\n").unwrap();
    let res = Command::new(env!("CARGO_BIN_EXE_stapelwerk"))
        .arg("compile")
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(res.status.code(), Some(1));
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(err.contains(":2:11: unknown variable 'x'"), "{err}");
}
