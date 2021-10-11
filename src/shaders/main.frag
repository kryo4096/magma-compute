#version 460

layout(push_constant) uniform PushConstants {
    int layer;
} push_constants;

layout(set=0, binding=0) uniform sampler2DArray tex;

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 f_color;

const ivec2 c[9] = {
    ivec2(0,0),
    ivec2(1,0),
    ivec2(0,1),
    ivec2(-1,0),
    ivec2(0,-1),
    ivec2(1,1),
    ivec2(-1,1),
    ivec2(1,-1),
    ivec2(-1,-1),
};

void main() {

    float f[9];

    float rho = 0;
    vec2 p = vec2(0);

    for(int i = 0; i < 9; i++) {
        f[i] = texture(tex, vec3(uv,i)).r;
        rho += 0.5 * f[i];
        p += 5.0 * f[i] * c[i];
    }



    f_color = vec4(clamp(p.x, 0.0, 1.0), clamp(-p.x, 0.0, 1.0), 0, 1.0);
 
}