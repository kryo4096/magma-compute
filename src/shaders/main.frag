#version 460

layout(push_constant) uniform PushConstants {
    uint screen_width;
    uint screen_height;
} push_constants;

layout(set=0, binding=0, rg32f) uniform image2DArray tex;

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 f_color;

void main() {

    f_color = vec4(0);

    f_color += imageLoad(tex, ivec3(uv * vec2(push_constants.screen_width, push_constants.screen_height), 0)).rrrr;
 
}