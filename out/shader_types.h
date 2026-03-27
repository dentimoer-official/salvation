#ifndef SHADER_TYPES_H
#define SHADER_TYPES_H

#ifdef __METAL_VERSION__
#  include <metal_stdlib>
   using namespace metal;
#else
#  include <simd/simd.h>
#endif

struct FrameUniforms {
#ifdef __METAL_VERSION__
    float4x4 projectionViewModel;
#else
    simd::float4x4 projectionViewModel;
#endif
};

#endif /* SHADER_TYPES_H */
