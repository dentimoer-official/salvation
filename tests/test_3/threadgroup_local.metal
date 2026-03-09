#include <metal_stdlib>
using namespace metal;

kernel void prefix_sum(
    device const float* input [[buffer(0)]],
    device float* output [[buffer(1)]],
    uint3 thread_pos [[thread_position_in_grid]],
    uint3 threadgroup_pos [[threadgroup_position_in_grid]],
    uint3 threads_per_threadgroup [[threads_per_threadgroup]]
) {
    // built-ins
    uint3 thread = thread_pos;
    uint3 threadgroup_id = threadgroup_pos;
    
    threadgroup float shared[32];
    auto i = thread.x;
    shared[i] = input[i];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    auto val = simd_sum(shared[i]);
    output[i] = val;
}

