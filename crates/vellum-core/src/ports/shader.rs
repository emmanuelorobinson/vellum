//! Runtime shaders for shader fills and effects. Our dialect is a small, documented subset; the compiler turns
//! it into what the active renderer runs. Today: SkSL for CanvasKit and skia-safe. Later: WGSL for Vello.

use std::fmt;

/// What the shader is compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderTarget {
    /// Skia's shading language.
    Sksl,
    /// WebGPU's shading language.
    Wgsl,
}

/// A shader the renderer can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledShader {
    /// What it was compiled for.
    pub target: ShaderTarget,
    /// The program, in the target's language.
    pub code: String,
}

/// Why a shader did not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderError {
    /// There was no program.
    Empty,
    /// The source does not parse or type-check.
    Invalid {
        /// What is wrong.
        message: String,
    },
    /// The source uses something outside our dialect or the target.
    Unsupported {
        /// What it uses.
        message: String,
    },
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("The shader is empty."),
            Self::Invalid { message } => write!(f, "The shader does not compile: {message}"),
            Self::Unsupported { message } => {
                write!(f, "The shader uses something unsupported: {message}")
            }
        }
    }
}

impl std::error::Error for ShaderError {}

/// Compiles shader source for a renderer.
pub trait ShaderCompiler {
    /// `src` compiled for `target`. The same input gives the same output.
    fn compile(&self, src: &str, target: ShaderTarget) -> Result<CompiledShader, ShaderError>;
}
