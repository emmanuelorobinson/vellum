//! Rounding for determinism (guide §9). Positions, sizes, radii, gaps and paddings are written to 1/100 px;
//! ratios, opacities and gradient stops to 1/1000; angles to 1/100 degree. It happens here and nowhere else:
//! live state keeps every digit it was given, and rounding twice is rounding once.

use crate::tree::{BuildError, Step, Tree, render_path};

/// How finely a kind of number is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantum {
    /// Lengths: positions, sizes, radii, gaps, paddings, offsets, blur, font sizes. 1/100 px.
    Pixels,
    /// Ratios: opacities, gradient stops, mesh coordinates, transform entries. 1/1000.
    Ratio,
    /// Angles. 1/100 degree.
    Degrees,
}

impl Quantum {
    fn scale(self) -> f64 {
        match self {
            Self::Ratio => 1000.0,
            Self::Pixels | Self::Degrees => 100.0,
        }
    }
}

/// Beyond this many steps a double has no fraction left to round, and scaling back could move it.
const LIMIT: f64 = 1_125_899_906_842_624.0; // 2^50

/// `value` rounded to `quantum`, halves away from zero. A value too large to have a fraction is returned as
/// it is, and so is the result of rounding it (that is what makes this idempotent). Never returns `-0`.
pub fn quantize(value: f64, quantum: Quantum) -> f64 {
    let scale = quantum.scale();
    let scaled = value * scale;
    if !scaled.is_finite() || scaled.abs() >= LIMIT {
        return value;
    }
    let rounded = scaled.round() / scale;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// The unit of a number called `key` in the file, whatever it sits in. `None` when the format gives it none.
///
/// This is a table of field names, and the test `quantization_table.rs` reads the schema to keep it complete:
/// a number added to the types without a line here fails that test.
pub fn quantum_of(key: &str) -> Option<Quantum> {
    Some(match key {
        "width" | "height" | "x" | "y" | "gap" | "padding" | "cornerRadius" | "radius" | "blur"
        | "spread" | "size" | "lineHeight" | "letterSpacing" | "dash" | "hug" | "fill" => {
            Quantum::Pixels
        }
        "opacity" | "position" | "innerRadius" | "innerRatio" | "zoom" | "crop" | "transform" => {
            Quantum::Ratio
        }
        "rotation" | "startAngle" | "sweepAngle" => Quantum::Degrees,
        _ => return None,
    })
}

/// Numbers the format defines but gives no unit: a number variable's value and a number prop's default can
/// be a length, a ratio or a count, depending on what uses them, so they are written as they are.
pub const EXACT_NUMBER_KEYS: &[&str] = &["default", "value"];

/// Keys under which numbers are the user's own: free-form metadata, shader uniforms, script inputs. A node's
/// `props` are too (a prop's name is the user's), except in an override, where `props` sets node properties.
fn is_free_form(keys: &[&str]) -> bool {
    keys.iter().enumerate().any(|(i, key)| match *key {
        "metadata" | "uniforms" | "inputs" => true,
        "props" => !(i >= 2 && keys[i - 2] == "overrides"),
        _ => false,
    })
}

fn quantum_at(path: &[Step]) -> Option<Quantum> {
    let keys: Vec<&str> = path
        .iter()
        .filter_map(|step| match step {
            Step::Key(key) => Some(key.as_str()),
            Step::Index(_) => None,
        })
        .collect();
    let (nearest, above) = keys.split_last()?;
    if is_free_form(&keys) {
        return None;
    }
    // A mesh gradient's points and handles are positions across the shape, 0 to 1, not pixels.
    if matches!(*nearest, "x" | "y") && above.iter().any(|k| matches!(*k, "points" | "handles")) {
        return Some(Quantum::Ratio);
    }
    quantum_of(nearest)
}

/// A number that cannot be written.
pub(crate) struct NotFinite(pub(crate) String);

/// Rounds every number in `tree` to its unit. Returns whether any number changed.
pub(crate) fn round_tree(tree: &mut Tree) -> Result<bool, NotFinite> {
    let mut path = Vec::new();
    walk(tree, &mut path)
}

fn walk(tree: &mut Tree, path: &mut Vec<Step>) -> Result<bool, NotFinite> {
    let mut changed = false;
    match tree {
        Tree::Float(value) => {
            if !value.is_finite() {
                return Err(NotFinite(render_path(path)));
            }
            if let Some(quantum) = quantum_at(path) {
                let rounded = quantize(*value, quantum);
                changed = rounded.to_bits() != value.to_bits();
                *value = rounded;
            }
        }
        Tree::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                path.push(Step::Index(index));
                changed |= walk(item, path)?;
                path.pop();
            }
        }
        Tree::Object(entries) => {
            for (key, item) in entries.iter_mut() {
                path.push(Step::Key(key.clone()));
                changed |= walk(item, path)?;
                path.pop();
            }
        }
        _ => {}
    }
    Ok(changed)
}

impl From<BuildError> for crate::CodecError {
    fn from(error: BuildError) -> Self {
        Self::Invalid {
            path: render_path(&error.steps()),
            position: None,
            message: error.message,
        }
    }
}
