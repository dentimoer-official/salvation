#version 450

layout(location = 0) in vec4 position;
layout(location = 1) in vec4 color;

layout(location = 0) out vec4 color_vert;

layout(set = 0, binding = 0) uniform FrameUniforms {
    mat4 projectionViewModel;
} uniforms;

void main() {
    color_vert = color;
    gl_Position = uniforms.projectionViewModel * position;
}
