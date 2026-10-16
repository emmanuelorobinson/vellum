use std::sync::Mutex;

use serde_json::{Value, json};
use vellum_core::ports::{
    SpecError, SpecFormat, TokenError, TokenExport, TokenFormat, TokenImport,
};
use vellum_core::tokens::{TokenOptions, TokenSet, TokenValue, TokenWarning};
use vellum_types::{Document, NodeId};

/// A token format that writes a description of the set and reads back only what it wrote.
#[derive(Default)]
pub struct MemoryTokens {
    written: Mutex<Vec<(String, TokenSet)>>,
}

impl TokenFormat for MemoryTokens {
    fn id(&self) -> &'static str {
        "memory"
    }
    fn label(&self) -> &'static str {
        "Memory"
    }
    fn file_name(&self) -> &'static str {
        "tokens.memory"
    }
    fn export(&self, set: &TokenSet, _: &TokenOptions) -> Result<TokenExport, TokenError> {
        let finite = |v: &TokenValue| !matches!(v, TokenValue::Number(n) if !n.is_finite());
        let mut warnings = Vec::new();
        let mut set = set.clone();
        set.tokens.retain(|token| {
            let keep = finite(&token.value) && token.modes.iter().all(|m| finite(&m.value));
            if !keep {
                warnings.push(TokenWarning::token(
                    &token.name,
                    "its number is not finite; left out.",
                ));
            }
            keep
        });
        let text = format!("{set:?}\n");
        let mut written = self.written.lock().expect("not poisoned");
        if !written.iter().any(|(known, _)| *known == text) {
            written.push((text.clone(), set.clone()));
        }
        Ok(TokenExport { text, warnings })
    }
    fn import(&self, text: &str, _: &TokenOptions) -> Result<TokenImport, TokenError> {
        let written = self.written.lock().expect("not poisoned");
        match written.iter().find(|(known, _)| known == text) {
            Some((_, set)) => Ok(TokenImport {
                set: set.clone(),
                warnings: vec![],
            }),
            None => Err(TokenError::new(
                "this is not a file the memory format wrote.",
            )),
        }
    }
}

/// A spec format that describes a layer by its id and lists no tokens.
pub struct MemorySpec;

impl SpecFormat for MemorySpec {
    fn id(&self) -> &'static str {
        "memory-spec"
    }
    fn build(&self, doc: &Document, frame: &NodeId) -> Result<Value, SpecError> {
        vellum_core::ops::find(doc, frame)
            .ok_or_else(|| SpecError::new(format!("there is no layer {frame}.")))?;
        Ok(json!({"format": self.id(), "frame": frame.as_str(), "tokens": []}))
    }
}
