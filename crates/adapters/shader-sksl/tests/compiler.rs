// TEST DATA: native CPU SkSL compiler; no GPU or network.
use vellum_core::ports::{ShaderCompiler, ShaderError, ShaderTarget};
use vellum_shader_sksl::SkslCompiler;
#[test]
fn contract_and_typed_errors() {
    vellum_contracts::suites::shader::run(&SkslCompiler);
    assert!(matches!(
        SkslCompiler.compile(
            "half4 main(float2 p) { return missing; }",
            ShaderTarget::Sksl
        ),
        Err(ShaderError::Invalid { .. })
    ));
    assert!(matches!(
        SkslCompiler.compile(
            "void main() { gl_FragColor = vec4(1.); }",
            ShaderTarget::Sksl
        ),
        Err(ShaderError::Unsupported { .. })
    ));
}
#[test]
fn runaway_and_compile_bombs_are_rejected_before_skia() {
    for src in [
        "half4 main(float2 p) { while(true) {} return half4(1); }",
        "half4 main(float2 p) { for(int i=0;i<100000;i++) {} return half4(1); }",
        "half4 f(float2 p) { return f(p); } half4 main(float2 p) { return f(p); }",
        "#define f main\nhalf4 f(float2 p) { return half4(1); }",
        "half4 main(float2 p) { float x[999999]; return half4(1); }",
    ] {
        assert!(
            SkslCompiler.compile(src, ShaderTarget::Sksl).is_err(),
            "{src}"
        );
    }
    assert!(
        SkslCompiler
            .compile(&" ".repeat(20000), ShaderTarget::Sksl)
            .is_err()
    );
}
#[test]
fn declarations_and_annotations_are_the_uniform_contract() {
    let info = vellum_core::shaders::inspect("// @color tint\nuniform float4 tint; uniform float strength; uniform float2 size; half4 main(float2 p) { float2 uv = p / size; return half4(tint * strength); }").unwrap();
    assert_eq!(info.len(), 3);
    assert_eq!(format!("{:?}", info[0].kind), "Color");
}
