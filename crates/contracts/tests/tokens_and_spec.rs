//! TEST DATA. The token and spec suites accept the real formats' fake and reject implementations that break
//! each rule.
use std::cell::Cell;

use serde_json::{Value, json};
use vellum_contracts::fakes::{MemorySpec, MemoryTokens};
use vellum_contracts::{samples, suites};
use vellum_core::ports::{
    SpecError, SpecFormat, TokenError, TokenExport, TokenFormat, TokenImport,
};
use vellum_core::spec::LayoutSpecV1;
use vellum_core::tokens::{TokenOptions, TokenSet, TokenValue};
use vellum_types::{Document, NodeId};

fn run(format: &dyn TokenFormat) {
    let set = samples::token_set();
    suites::tokens::format(format, &set, &TokenOptions::for_themes(&set.themes));
}

#[test]
fn layout_spec_v1_passes_the_spec_suite() {
    suites::spec::format(
        &LayoutSpecV1,
        &samples::document(),
        &samples::node_id("frame00001"),
    );
}

/// Wraps the memory format and breaks one rule.
struct Broken {
    inner: MemoryTokens,
    how: &'static str,
    calls: Cell<u32>,
}

impl Broken {
    fn new(how: &'static str) -> Self {
        Self {
            inner: MemoryTokens::default(),
            how,
            calls: Cell::new(0),
        }
    }
}

impl TokenFormat for Broken {
    fn id(&self) -> &'static str {
        if self.how == "no id" { "" } else { "broken" }
    }
    fn label(&self) -> &'static str {
        "Broken"
    }
    fn file_name(&self) -> &'static str {
        "tokens.broken"
    }
    fn export(&self, set: &TokenSet, options: &TokenOptions) -> Result<TokenExport, TokenError> {
        self.calls.set(self.calls.get() + 1);
        let mut set = set.clone();
        match self.how {
            "unstable" => {
                set.themes.insert(format!("n{}", self.calls.get()), vec![]);
            }
            "silent loss" => {
                set.tokens.pop();
            }
            _ => {}
        }
        let not_finite = set
            .tokens
            .iter()
            .any(|t| matches!(t.value, TokenValue::Number(n) if !n.is_finite()));
        let mut out = self.inner.export(&set, options)?;
        if matches!(self.how, "writes nan" | "writes nan and says so") && not_finite {
            out.text.push_str("NaN\n");
            if self.how == "writes nan" {
                out.warnings.clear();
            }
        }
        Ok(out)
    }
    fn import(&self, text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError> {
        if self.how == "makes tokens up" && !text.contains("TokenSet") {
            let mut set = samples::token_set();
            set.themes.clear();
            return Ok(TokenImport {
                set,
                warnings: vec![],
            });
        }
        let text = text.replace("NaN\n", "");
        self.inner.import(&text, options)
    }
}

#[test]
fn the_token_suite_accepts_the_memory_format() {
    run(&MemoryTokens::default());
}

#[test]
#[should_panic(expected = "id, label and file name")]
fn token_suite_rejects_a_format_without_an_id() {
    run(&Broken::new("no id"));
}

#[test]
#[should_panic(expected = "different text")]
fn token_suite_rejects_unstable_output() {
    run(&Broken::new("unstable"));
}

#[test]
#[should_panic(expected = "did not give the same tokens")]
fn token_suite_rejects_a_silent_loss() {
    run(&Broken::new("silent loss"));
}

#[test]
#[should_panic(expected = "tokens made up")]
fn token_suite_rejects_tokens_made_up_from_junk() {
    run(&Broken::new("makes tokens up"));
}

#[test]
#[should_panic(expected = "was not reported")]
fn token_suite_rejects_a_number_dropped_without_a_word() {
    run(&Broken::new("writes nan"));
}

#[test]
#[should_panic(expected = "was written")]
fn token_suite_rejects_a_number_written_that_is_not_finite() {
    run(&Broken::new("writes nan and says so"));
}

/// A spec format that breaks one rule.
struct BrokenSpec(&'static str);

impl SpecFormat for BrokenSpec {
    fn id(&self) -> &'static str {
        "broken-spec"
    }
    fn build(&self, doc: &Document, frame: &NodeId) -> Result<Value, SpecError> {
        if self.0 != "invents a layer" {
            vellum_core::ops::find(doc, frame).ok_or_else(|| SpecError::new("no such layer"))?;
        }
        Ok(match self.0 {
            "wrong name" => json!({"format": "other", "tokens": []}),
            "unlisted token" => json!({"format": "broken-spec", "tokens": [],
                "root": {"fills": [{"color": {"$token": "color/primary"}}]}}),
            "no tokens list" => json!({"format": "broken-spec"}),
            _ => json!({"format": "broken-spec", "tokens": []}),
        })
    }
}

fn spec(how: &'static str) {
    suites::spec::format(
        &BrokenSpec(how),
        &samples::document(),
        &samples::node_id("frame00001"),
    );
}

#[test]
fn the_spec_suite_accepts_the_memory_format() {
    suites::spec::format(
        &MemorySpec,
        &samples::document(),
        &samples::node_id("frame00001"),
    );
    spec("fine");
}

#[test]
#[should_panic(expected = "does not name its format")]
fn spec_suite_rejects_a_wrong_format_name() {
    spec("wrong name");
}

#[test]
#[should_panic(expected = "tokens used but not listed")]
fn spec_suite_rejects_an_unlisted_token() {
    spec("unlisted token");
}

#[test]
#[should_panic(expected = "no tokens list")]
fn spec_suite_rejects_a_missing_token_list() {
    spec("no tokens list");
}

#[test]
#[should_panic(expected = "was described")]
fn spec_suite_rejects_describing_a_missing_layer() {
    spec("invents a layer");
}
