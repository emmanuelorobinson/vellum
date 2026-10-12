//! `ShaderCompiler`.

use vellum_core::ports::{ShaderCompiler, ShaderTarget};

const PROGRAM: &str = "half4 main(float2 p) { return half4(1.0); }";

/// Nothing compiles to nothing, a result names the target it was asked for, and the same input gives the same
/// output.
pub fn run(compiler: &dyn ShaderCompiler) {
    for target in [ShaderTarget::Sksl, ShaderTarget::Wgsl] {
        for empty in ["", " \n\t"] {
            assert!(
                compiler.compile(empty, target).is_err(),
                "ShaderCompiler: {empty:?} compiled for {target:?}"
            );
        }
    }

    let first = compiler.compile(PROGRAM, ShaderTarget::Sksl);
    let second = compiler.compile(PROGRAM, ShaderTarget::Sksl);
    assert_eq!(
        first, second,
        "ShaderCompiler: the same source gave two results"
    );
    let compiled =
        first.unwrap_or_else(|e| panic!("ShaderCompiler: a plain program for SkSL failed: {e}"));
    assert_eq!(
        compiled.target,
        ShaderTarget::Sksl,
        "ShaderCompiler: the target changed"
    );
    assert!(
        !compiled.code.trim().is_empty(),
        "ShaderCompiler: the compiled program is empty"
    );

    if let Ok(wgsl) = compiler.compile(PROGRAM, ShaderTarget::Wgsl) {
        assert_eq!(
            wgsl.target,
            ShaderTarget::Wgsl,
            "ShaderCompiler: the target changed"
        );
    }
}
