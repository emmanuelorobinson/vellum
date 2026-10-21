// @color tint
uniform float4 tint;
uniform float strength;
uniform shader content;
uniform float2 size;
half4 main(float2 p) {
    half4 pixel = content.eval(p);
    return pixel * half4(tint.rgb * strength, 1.0);
}
