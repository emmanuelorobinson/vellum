// @color tint
uniform float4 tint;
uniform float scale;
uniform float2 size;
half4 main(float2 p) {
    float2 cell = floor(p / size * scale);
    float alternate = mod(cell.x + cell.y, 2.0);
    return half4(mix(float4(1.0), tint, alternate));
}
