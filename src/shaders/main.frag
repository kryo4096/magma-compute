#version 460

layout(set=0, binding=0) uniform sampler2DArray tex;

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 f_color;

void main() {

    f_color = vec4(0);

    f_color += texture(tex, vec3(uv, 0)).rrrr;
 
}