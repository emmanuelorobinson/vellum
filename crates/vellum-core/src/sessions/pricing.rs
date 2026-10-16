//! Estimated cost at API rates (guide §9): token counts times the prices in a local `pricing.json`, a versioned
//! data file the daemon ships. Money is `u64` micro-USD and is always shown as an estimate. A model or token kind
//! the file does not price has no estimate rather than a guessed one.
//!
//! ```json
//! { "version": 1, "checked": "2026-10-16", "source": "…",
//!   "models": [ { "id": "m", "aliases": ["m-latest"], "input": 4000000, "output": 20000000,
//!                 "cacheRead": 200000, "cacheWrite": 5000000 } ] }
//! ```
//!
//! Prices are micro-USD per million tokens, so $4 per million tokens is `4000000`.
use crate::ports::Tokens;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

pub const PRICING_VERSION: u64 = 1;

/// Micro-USD per million tokens, by kind. A kind that is `None` is not priced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prices {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pricing {
    /// The date the prices were last checked, as written in the file.
    pub checked: String,
    models: BTreeMap<String, Prices>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PricingError(pub String);
impl fmt::Display for PricingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pricing.json: {}", self.0)
    }
}
impl std::error::Error for PricingError {}

fn fail<T>(message: impl Into<String>) -> Result<T, PricingError> {
    Err(PricingError(message.into()))
}

fn only(object: &Map<String, Value>, allowed: &[&str], at: &str) -> Result<(), PricingError> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => fail(format!("unknown field `{key}` in {at}")),
        None => Ok(()),
    }
}

impl Pricing {
    pub fn from_json(text: &str) -> Result<Self, PricingError> {
        let root: Value = serde_json::from_str(text).map_err(|e| PricingError(e.to_string()))?;
        let Some(root) = root.as_object() else {
            return fail("not an object");
        };
        only(
            root,
            &["version", "checked", "source", "models"],
            "the file",
        )?;
        if root.get("version").and_then(Value::as_u64) != Some(PRICING_VERSION) {
            return fail(format!("version must be {PRICING_VERSION}"));
        }
        let Some(checked) = root.get("checked").and_then(Value::as_str) else {
            return fail("`checked` must be a date");
        };
        let Some(entries) = root.get("models").and_then(Value::as_array) else {
            return fail("`models` must be a list");
        };
        let mut models = BTreeMap::new();
        for (index, entry) in entries.iter().enumerate() {
            let at = format!("models[{index}]");
            let Some(entry) = entry.as_object() else {
                return fail(format!("{at} is not an object"));
            };
            only(
                entry,
                &[
                    "id",
                    "provider",
                    "aliases",
                    "input",
                    "output",
                    "cacheRead",
                    "cacheWrite",
                    "note",
                ],
                &at,
            )?;
            let price = |key: &str| -> Result<Option<u64>, PricingError> {
                match entry.get(key) {
                    None => Ok(None),
                    Some(value) => value
                        .as_u64()
                        .map(Some)
                        .ok_or_else(|| PricingError(format!("{at}.{key} must be a whole number"))),
                }
            };
            let prices = Prices {
                input: price("input")?,
                output: price("output")?,
                cache_read: price("cacheRead")?,
                cache_write: price("cacheWrite")?,
            };
            let Some(id) = entry
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            else {
                return fail(format!("{at}.id is missing"));
            };
            let mut names = vec![id];
            if let Some(aliases) = entry.get("aliases") {
                let Some(aliases) = aliases.as_array() else {
                    return fail(format!("{at}.aliases must be a list"));
                };
                for alias in aliases {
                    match alias.as_str().filter(|a| !a.is_empty()) {
                        Some(alias) => names.push(alias),
                        None => return fail(format!("{at}.aliases must be names")),
                    }
                }
            }
            for name in names {
                if models.insert(name.to_owned(), prices).is_some() {
                    return fail(format!("`{name}` is priced twice"));
                }
            }
        }
        Ok(Self {
            checked: checked.to_owned(),
            models,
        })
    }

    /// The prices of `model`, by exact id or alias.
    pub fn prices(&self, model: &str) -> Option<Prices> {
        self.models.get(model).copied()
    }

    /// The estimated cost of `tokens` on `model`, in micro-USD, rounded half up. `None` when the model is not
    /// priced or a kind it used has no price.
    pub fn estimate(&self, model: &str, tokens: Tokens) -> Option<u64> {
        let prices = self.prices(model)?;
        let mut total: u128 = 0;
        for (count, price) in [
            (tokens.input, prices.input),
            (tokens.output, prices.output),
            (tokens.cache_read, prices.cache_read),
            (tokens.cache_write, prices.cache_write),
        ] {
            if count > 0 {
                total += u128::from(count) * u128::from(price?);
            }
        }
        u64::try_from((total + 500_000) / 1_000_000).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{
        "version": 1, "checked": "2026-10-16", "source": "TEST DATA",
        "models": [
            {"id": "big", "aliases": ["big-latest"], "input": 4000000, "output": 20000000,
             "cacheRead": 200000, "cacheWrite": 5000000},
            {"id": "plain", "input": 100000, "output": 500000}
        ]
    }"#;

    fn tokens(input: u64, output: u64, cache_read: u64, cache_write: u64) -> Tokens {
        Tokens {
            input,
            output,
            cache_read,
            cache_write,
        }
    }

    #[test]
    fn estimates_by_kind_in_micro_usd() {
        let pricing = Pricing::from_json(FILE).unwrap();
        assert_eq!(pricing.checked, "2026-10-16");
        // 1,000 input at $4/M = 4,000 µ$; 500 output at $20/M = 10,000 µ$.
        assert_eq!(
            pricing.estimate("big", tokens(1_000, 500, 0, 0)),
            Some(14_000)
        );
        // Cache kinds are priced separately: 10,000 reads at $0.20/M = 2,000; 1,000 writes at $5/M = 5,000.
        assert_eq!(
            pricing.estimate("big", tokens(0, 0, 10_000, 1_000)),
            Some(7_000)
        );
        assert_eq!(
            pricing.estimate("big-latest", tokens(1_000, 0, 0, 0)),
            Some(4_000)
        );
        assert_eq!(pricing.estimate("big", Tokens::default()), Some(0));
    }

    #[test]
    fn rounds_once_over_the_sum() {
        let pricing = Pricing::from_json(FILE).unwrap();
        // 3 input tokens at $0.10/M = 0.3 µ$ and 1 output at $0.50/M = 0.5 µ$: 0.8 rounds to 1, where rounding
        // each kind first would give 0 + 1.
        assert_eq!(pricing.estimate("plain", tokens(3, 1, 0, 0)), Some(1));
        assert_eq!(pricing.estimate("plain", tokens(4, 0, 0, 0)), Some(0));
        assert_eq!(pricing.estimate("plain", tokens(5, 0, 0, 0)), Some(1));
        // Large counts do not overflow; a cost beyond u64 has no estimate.
        assert_eq!(
            pricing.estimate("big", tokens(u64::MAX / 8, 0, 0, 0)),
            Some(9_223_372_036_854_775_804)
        );
        assert_eq!(pricing.estimate("big", tokens(u64::MAX, 0, 0, 0)), None);
    }

    #[test]
    fn unknown_models_and_unpriced_kinds_have_no_estimate() {
        let pricing = Pricing::from_json(FILE).unwrap();
        assert_eq!(pricing.estimate("nobody", tokens(1, 1, 0, 0)), None);
        assert_eq!(pricing.estimate("plain", tokens(1, 0, 1, 0)), None);
        assert_eq!(pricing.estimate("plain", tokens(1, 0, 0, 1)), None);
        // An unpriced kind that was not used does not matter.
        assert_eq!(pricing.estimate("plain", tokens(10, 0, 0, 0)), Some(1));
    }

    #[test]
    fn refuses_malformed_files() {
        for (text, needle) in [
            (r#"{"version": 2, "checked": "x", "models": []}"#, "version"),
            (r#"{"version": 1, "models": []}"#, "checked"),
            (
                r#"{"version": 1, "checked": "x", "models": [], "extra": 1}"#,
                "extra",
            ),
            (
                r#"{"version": 1, "checked": "x", "models": [{"id": "a", "input": 1.5}]}"#,
                "whole number",
            ),
            (
                r#"{"version": 1, "checked": "x", "models": [{"id": "a", "input": -1}]}"#,
                "whole number",
            ),
            (
                r#"{"version": 1, "checked": "x", "models": [{"id": "a"}, {"id": "b", "aliases": ["a"]}]}"#,
                "twice",
            ),
            (
                r#"{"version": 1, "checked": "x", "models": [{"input": 1}]}"#,
                "id",
            ),
            (
                r#"{"version": 1, "checked": "x", "models": [{"id": "a", "price": 1}]}"#,
                "price",
            ),
        ] {
            let error = Pricing::from_json(text).unwrap_err();
            assert!(error.0.contains(needle), "{text}: {error}");
        }
    }
}
