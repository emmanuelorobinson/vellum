//! Vellum's bounded expression dialect → SkSL. No GLSL rewriting or ambient resources.
use vellum_core::ports::{CompiledShader, ShaderCompiler, ShaderError, ShaderTarget};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
#[derive(Default)]
pub struct SkslCompiler;
impl ShaderCompiler for SkslCompiler {
    fn compile(&self, src: &str, target: ShaderTarget) -> Result<CompiledShader, ShaderError> {
        vellum_core::shaders::inspect(src)?;
        if target != ShaderTarget::Sksl {
            return Err(ShaderError::Unsupported {
                message: "WGSL is not implemented".into(),
            });
        }
        skia_safe::RuntimeEffect::make_for_shader(src, None)
            .map_err(|message| ShaderError::Invalid { message })?;
        Ok(CompiledShader {
            target,
            code: src.to_owned(),
        })
    }
}
