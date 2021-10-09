#version 460

layout(location = 0) out vec2 uv;

const int indices[] = {
    0, 1, 2, 0, 2, 3
};

const vec2 vertices[] = {
    vec2(1.0,1.0),
    vec2(-1.0,1.0),
    vec2(-1.0,-1.0),
    vec2(1.0,-1.0)
};

void main() {

    gl_Position = vec4(0.95 * vertices[indices[gl_VertexIndex]], 0, 1);

    uv = vertices[indices[gl_VertexIndex]]*0.5 + 0.5;
}