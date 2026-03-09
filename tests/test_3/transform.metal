#include <metal_stdlib>
using namespace metal;

struct Matrix4 {
    float4 col0;
    float4 col1;
    float4 col2;
    float4 col3;
};

constant float PI = 3.14159f;

kernel void transform(
    device Matrix4* matrices [[buffer(0)]],
    device float* out [[buffer(1)]],
    uint3 thread_pos [[thread_position_in_grid]],
    uint3 threadgroup_pos [[threadgroup_position_in_grid]],
    uint3 threads_per_threadgroup [[threads_per_threadgroup]]
) {
    // built-ins
    uint3 thread = thread_pos;
    uint3 threadgroup_id = threadgroup_pos;
    
    auto i = thread.x;
    auto scale = PI;
    out[i] = scale;
}

