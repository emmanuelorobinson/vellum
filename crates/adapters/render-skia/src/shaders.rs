//! Host-supplied shader text only. Admission is pure, Skia type-checks, no path is opened here.
use crate::{Resources, matrix};
use skia_safe::{self as sk, Rect, runtime_effect::RuntimeShaderBuilder};
use std::collections::BTreeMap;
use vellum_core::{
    ports::ShaderError,
    shaders::{UniformKind, inspect},
};
use vellum_types::display::RenderUniform;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderDiagnostic {
    pub src: String,
    pub error: ShaderError,
}
fn error(message: &str) -> ShaderError {
    ShaderError::Invalid {
        message: message.into(),
    }
}
fn builder(
    src: &str,
    uniforms: &BTreeMap<String, RenderUniform>,
    bounds: Rect,
    resources: &Resources,
    effect: bool,
) -> Result<RuntimeShaderBuilder, ShaderError> {
    let code = resources
        .shaders
        .get(src)
        .ok_or_else(|| error("Shader source is unavailable"))?;
    let info = inspect(code)?;
    let has_content = info.iter().any(|u| u.kind == UniformKind::Content);
    if has_content != effect {
        return Err(error(
            "Effects require uniform shader content; fills cannot sample content",
        ));
    }
    for name in uniforms.keys() {
        if name == "size" || name == "content" || !info.iter().any(|u| &u.name == name) {
            return Err(error("Unknown or reserved uniform"));
        }
    }
    let compiled = sk::RuntimeEffect::make_for_shader(code.as_ref(), None)
        .map_err(|message| ShaderError::Invalid { message })?;
    let mut builder = RuntimeShaderBuilder::new(compiled);
    for u in info {
        if u.kind == UniformKind::Content {
            continue;
        }
        let v = if u.name == "size" {
            RenderUniform::Vector(vec![bounds.width().max(1.), bounds.height().max(1.)])
        } else {
            uniforms
                .get(&u.name)
                .cloned()
                .ok_or_else(|| error(&format!("Missing uniform {}", u.name)))?
        };
        let floats = match (u.kind, v) {
            (UniformKind::Boolean, RenderUniform::Bool(b)) => {
                builder
                    .set_uniform_int(&u.name, &[i32::from(b)])
                    .map_err(|_| error("Uniform type mismatch"))?;
                continue;
            }
            (UniformKind::Number, RenderUniform::Number(n)) => vec![n],
            (UniformKind::Color, RenderUniform::Color(c)) => {
                c.map(|v| f32::from(v) / 255.).to_vec()
            }
            (UniformKind::Vector(n), RenderUniform::Vector(v)) if v.len() == usize::from(n) => v,
            _ => return Err(error("Uniform type mismatch")),
        };
        if floats.iter().any(|n| !n.is_finite() || n.abs() > 1.0e6) {
            return Err(error("Uniform exceeds finite range"));
        }
        builder
            .set_uniform_float(&u.name, &floats)
            .map_err(|_| error("Uniform type mismatch"))?;
    }
    Ok(builder)
}
pub(crate) fn fill(
    src: &str,
    uniforms: &BTreeMap<String, RenderUniform>,
    bounds: Rect,
    resources: &Resources,
) -> Result<sk::Shader, ShaderError> {
    builder(src, uniforms, bounds, resources, false)?
        .make_shader(&matrix([1., 0., 0., 1., bounds.left, bounds.top]))
        .ok_or_else(|| error("Shader could not be instantiated"))
}
pub(crate) fn effect(
    src: &str,
    uniforms: &BTreeMap<String, RenderUniform>,
    bounds: Rect,
    resources: &Resources,
    input: Option<sk::ImageFilter>,
) -> Result<sk::ImageFilter, ShaderError> {
    let builder = builder(src, uniforms, bounds, resources, true)?;
    sk::image_filters::runtime_shader(&builder, "content", input)
        .ok_or_else(|| error("Shader effect could not be instantiated"))
}
