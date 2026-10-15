//! Bounded, copied JSON values in both directions; no handles or host objects cross this boundary.
use serde_json::Value;
use vellum_core::ports::{ScriptError, ScriptValue};

pub fn decode(json: &str) -> Result<ScriptValue, ScriptError> {
    fn convert(v: Value) -> ScriptValue {
        match v {
            Value::Null => ScriptValue::Null,
            Value::Bool(v) => ScriptValue::Bool(v),
            Value::Number(v) => ScriptValue::Number(v.as_f64().expect("JSON finite number")),
            Value::String(v) => ScriptValue::Text(v),
            Value::Array(v) => ScriptValue::List(v.into_iter().map(convert).collect()),
            Value::Object(v) => {
                ScriptValue::Map(v.into_iter().map(|(k, v)| (k, convert(v))).collect())
            }
        }
    }
    serde_json::from_str(json)
        .map(convert)
        .map_err(|_| ScriptError::Forbidden {
            what: "invalid JSON data".into(),
        })
}
pub fn encode(value: &ScriptValue, limit: usize) -> Result<String, ScriptError> {
    fn convert(value: &ScriptValue, depth: usize, left: &mut usize) -> Result<Value, ScriptError> {
        if depth > 32 {
            return Err(ScriptError::Forbidden {
                what: "boundary depth".into(),
            });
        }
        *left = left.checked_sub(8).ok_or(ScriptError::MemoryLimit)?;
        Ok(match value {
            ScriptValue::Null => Value::Null,
            ScriptValue::Bool(v) => Value::Bool(*v),
            ScriptValue::Number(v) => serde_json::Number::from_f64(*v).map(Value::Number).ok_or(
                ScriptError::Forbidden {
                    what: "nonfinite host value".into(),
                },
            )?,
            ScriptValue::Text(v) => {
                *left = left.checked_sub(v.len()).ok_or(ScriptError::MemoryLimit)?;
                Value::String(v.clone())
            }
            ScriptValue::List(v) => Value::Array(
                v.iter()
                    .map(|v| convert(v, depth + 1, left))
                    .collect::<Result<_, _>>()?,
            ),
            ScriptValue::Map(v) => Value::Object(
                v.iter()
                    .map(|(k, v)| {
                        if matches!(k.as_str(), "__proto__" | "constructor" | "prototype") {
                            return Err(ScriptError::Forbidden {
                                what: "prototype host data".into(),
                            });
                        }
                        *left = left.checked_sub(k.len()).ok_or(ScriptError::MemoryLimit)?;
                        Ok((k.clone(), convert(v, depth + 1, left)?))
                    })
                    .collect::<Result<_, _>>()?,
            ),
        })
    }
    let mut remaining = limit;
    let json = serde_json::to_string(&convert(value, 0, &mut remaining)?).expect("JSON value");
    if json.len() > limit {
        Err(ScriptError::MemoryLimit)
    } else {
        Ok(json)
    }
}
