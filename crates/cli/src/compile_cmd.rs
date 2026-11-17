//! `compile`: Setzwerk source (`.sz`) to Stapelwerk assembly (`docs/SETZWERK.md`).

use crate::args::Parsed;
use crate::program_cmds::{print_diags, read_text};
use stapelwerk_core::asm::{MnemonicFilter, assemble};
use std::io::Write;
use std::path::Path;

pub fn cmd_compile(p: &Parsed, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let path = &p.positional[0];
    let Some(src) = read_text(path, err) else {
        return 1;
    };
    // Only the file name goes into the header comment, never the directory.
    let name = Path::new(path)
        .file_name()
        .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
    let asm = match stapelwerk_setz::compile(&src, &name) {
        Ok(a) => a,
        Err(e) => {
            let _ = writeln!(err, "{path}:{e}");
            return 1;
        }
    };
    // The output must assemble (it can only fail on the size limits of the machine).
    let program = match assemble(&asm, None, &MnemonicFilter::All) {
        Ok(prog) => prog,
        Err(diags) => {
            let _ = writeln!(
                err,
                "stapelwerk: the compiled program does not fit the machine:"
            );
            print_diags(&format!("{path} (compiled)"), None, &diags, err);
            return 1;
        }
    };
    match p.value("-o") {
        Some(target) => match std::fs::write(target, &asm) {
            Ok(()) => {
                let _ = writeln!(
                    out,
                    "wrote {target}: {} code words, {} data bytes",
                    program.words.len(),
                    program.data.len()
                );
                0
            }
            Err(e) => {
                let _ = writeln!(err, "stapelwerk: cannot write '{target}': {e}");
                1
            }
        },
        None => match out.write_all(asm.as_bytes()) {
            Ok(()) => 0,
            Err(_) => 1,
        },
    }
}
