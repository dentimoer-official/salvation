#include <metal_stdlib>
using namespace metal;

struct Particle {
    float mass;
};

constant uint MAX_THREADS = 1024;

kernel void update(
    device Particle* particles [[buffer(0)]],
    uint3 thread_pos [[thread_position_in_grid]],
    uint3 threadgroup_pos [[threadgroup_position_in_grid]],
    uint3 threads_per_threadgroup [[threads_per_threadgroup]]
) {
    // built-ins
    uint3 thread = thread_pos;
    uint3 threadgroup_id = threadgroup_pos;
    
    auto i = thread.x;
}

