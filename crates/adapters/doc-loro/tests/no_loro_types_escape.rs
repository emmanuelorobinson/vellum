//! No Loro type escapes the adapter (ARCHITECTURE §2.3): nothing public in the crate names one, and the crate
//! does not re-export Loro. The compiler holds the rest: everything Loro sits in private fields and private
//! modules.

use std::path::Path;

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("the source directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("a source file");
            out.push((path.display().to_string(), text));
        }
    }
}

#[test]
fn nothing_public_names_a_loro_type() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    for (path, text) in files {
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            let public = code.starts_with("pub ") && !code.starts_with("pub(");
            if public {
                let named = code.replace("LoroDocStore", "");
                assert!(
                    !named.contains("loro") && !named.contains("Loro"),
                    "{path}:{}: a public item names a Loro type: {code}",
                    n + 1
                );
            }
            assert!(
                !(code.starts_with("pub use") && code.contains("loro")),
                "{path}:{}: the crate re-exports Loro",
                n + 1
            );
        }
    }
}
