//! Rewrites `.vellum` files in canonical form: `cargo run -p vellum-codec-json --example canonicalize -- [--check] FILE...`.
//! With `--check` it writes nothing and exits non-zero if any file is not already canonical. This is how the
//! fixtures under `fixtures/documents/` are produced.

use std::process::ExitCode;

use vellum_codec_json::{decode, encode};

fn main() -> ExitCode {
    let mut check = false;
    let mut failed = false;
    for arg in std::env::args().skip(1) {
        if arg == "--check" {
            check = true;
            continue;
        }
        let bytes = match std::fs::read(&arg) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{arg}: {error}");
                failed = true;
                continue;
            }
        };
        let canonical = decode(&bytes).and_then(|doc| encode(&doc));
        match canonical {
            Ok(canonical) if canonical == bytes => {}
            Ok(canonical) if check => {
                eprintln!("{arg}: not canonical");
                let _ = canonical;
                failed = true;
            }
            Ok(canonical) => {
                if let Err(error) = std::fs::write(&arg, canonical) {
                    eprintln!("{arg}: {error}");
                    failed = true;
                }
            }
            Err(error) => {
                eprintln!("{arg}: {error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
