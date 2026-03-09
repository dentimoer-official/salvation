#include <metal_stdlib>
using namespace metal;

kernel void add(
    device const float* a [[buffer(0)]],
    device const float* b [[buffer(1)]],
    device float* out [[buffer(2)]],
    uint3 thread_pos [[thread_position_in_grid]],
    uint3 threadgroup_pos [[threadgroup_position_in_grid]],
    uint3 threads_per_threadgroup [[threads_per_threadgroup]]
) {
    // built-ins
    uint3 thread = thread_pos;
    uint3 threadgroup_id = threadgroup_pos;
    
    auto i = thread.x;
    out[i] = (a[i] + b[i]);
}

