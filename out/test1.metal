#include <metal_stdlib>
using namespace metal;

struct FrameUniforms {
    float4x4 projectionViewModel;
};

struct VertIn {
    float4 position [[attribute(0)]];
    float4 color [[attribute(1)]];
};

struct VertOut {
    float4 position [[position]];
    float4 color;
};

vertex VertOut vert(VertIn in [[stage_in]], constant FrameUniforms& uniforms [[buffer(1)]]) {
    VertOut out;
    out.position = in.position;
    out.color = in.color;
    out.position = in.position;
    return out;
}

fragment float4 frag(VertOut out [[stage_in]]) {
    return out.color;
}

