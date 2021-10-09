#version 460

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = vec4(uv, 0, 1);
}