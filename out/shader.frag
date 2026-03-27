#version 450

layout(location = 0) in vec4 color_vert;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 color = color_vert;
    outColor = color;
}
