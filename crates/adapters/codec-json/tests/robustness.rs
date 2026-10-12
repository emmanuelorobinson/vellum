//! The decoder meets whatever is on disk: truncated saves, merge conflicts, hand edits, hostile input. It must
//! never panic, and whatever it accepts must save and load again to the same bytes. The cargo-fuzz target in
//! `fuzz/` checks the same thing with coverage guidance; this runs a fixed corpus on every `cargo test`.

use std::path::{Path, PathBuf};

use vellum_codec_json::{decode, encode};

fn fixtures() -> Vec<Vec<u8>> {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/documents");
    std::fs::read_dir(dir)
        .expect("fixtures")
        .map(|e| std::fs::read(e.expect("entry").path()).expect("readable"))
        .collect()
}

/// What the fuzz target asserts.
fn check(bytes: &[u8]) {
    let Ok(doc) = decode(bytes) else { return };
    let saved = encode(&doc).expect("a document that was read can be written");
    let again = decode(&saved).expect("what was written reads");
    assert_eq!(
        encode(&again).expect("encode"),
        saved,
        "saving is not stable"
    );
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(original: &[u8], rng: &mut Rng) -> Vec<u8> {
    const TOKENS: &[&str] = &[
        "{", "}", "[", "]", "\"", ",", ":", "0", "-", "1e400", "1e-400", "0.5", "null", "true",
        "\\", "\\u0000", "\\ud800", "\"type\"", "\"id\"", "\"$var\"", "\u{FEFF}", "\r\n",
    ];
    let mut bytes = original.to_vec();
    for _ in 0..=rng.below(4) {
        if bytes.is_empty() {
            break;
        }
        let at = rng.below(bytes.len());
        match rng.below(6) {
            0 => bytes[at] ^= 1 << rng.below(8),
            1 => bytes.truncate(at),
            2 => {
                let end = (at + rng.below(40)).min(bytes.len());
                bytes.drain(at..end);
            }
            3 => {
                let end = (at + rng.below(60)).min(bytes.len());
                let copy = bytes[at..end].to_vec();
                bytes.splice(at..at, copy);
            }
            4 => {
                let token = TOKENS[rng.below(TOKENS.len())].as_bytes().to_vec();
                bytes.splice(at..at, token);
            }
            _ => bytes[at] = rng.next() as u8,
        }
    }
    bytes
}

#[test]
fn mutated_fixtures_never_panic_and_whatever_reads_saves_stably() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for original in fixtures() {
        check(&original);
        for _ in 0..400 {
            check(&mutate(&original, &mut rng));
        }
    }
}

#[test]
fn hostile_inputs_are_refused_or_handled() {
    let meta = r#""meta": {"formatVersion": 1, "name": "x", "createdWith": "y"}"#;
    let cases: Vec<String> = vec![
        String::new(),
        "\u{0}".into(),
        "{".into(),
        format!("{{{meta}, \"pages\": [], \"pages\": []}}"),
        format!("{{{meta}, \"pages\": [{{\"id\": \"a\", \"name\": \"\\ud800\"}}]}}"),
        format!("{{{meta}, \"pages\": [{{\"id\": \"a\", \"name\": \"\\u0000nul\"}}]}}"),
        format!(
            "{{{meta}, \"pages\": [{{\"id\": \"a\", \"name\": \"x\", \"children\": [{{\"type\": \"rect\", \"id\": \"b\", \"name\": \"n\", \"width\": 1e400, \"height\": 1}}]}}]}}"
        ),
        format!(
            "{{{meta}, \"pages\": [{{\"id\": \"a\", \"name\": \"x\", \"children\": [{{\"type\": \"rect\", \"id\": \"b\", \"name\": \"n\", \"width\": 1e308, \"height\": -1e308, \"opacity\": 5e-324}}]}}]}}"
        ),
        format!(
            "{{{meta}, \"pages\": [{{\"id\": \"a\", \"name\": \"{}\"}}]}}",
            "x".repeat(1 << 20)
        ),
        "{\"meta\": {\"formatVersion\": 99999999999999999999, \"name\": \"x\", \"createdWith\": \"y\"}, \"pages\": []}".to_owned(),
        "[".repeat(200_000),
        "{\"a\":".repeat(200_000),
    ];
    for case in cases {
        check(case.as_bytes());
    }
}
