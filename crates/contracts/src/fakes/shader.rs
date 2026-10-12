use vellum_core::ports::{CompiledShader, ShaderCompiler, ShaderError, ShaderTarget};

/// Hands the source back as the compiled program, for whatever target. Only an empty source is refused.
#[derive(Debug, Default, Clone, Copy)]
pub struct PassthroughShaderCompiler;

impl ShaderCompiler for PassthroughShaderCompiler {
    fn compile(&self, src: &str, target: ShaderTarget) -> Result<CompiledShader, ShaderError> {
        if src.trim().is_empty() {
            return Err(ShaderError::Empty);
        }
        Ok(CompiledShader {
            target,
            code: src.to_owned(),
        })
    }
}
