#version 460

layout(push_constant) uniform PushConstants {
    float brightness;
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

vec3 hsv2rgb(vec3 c)
{
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}



void main() {

    float f[9];

    float rho = 0;
    vec2 p = vec2(0);
    float P = 0;

    for(int i = 0; i < 9; i++) {
        f[i] = texture(tex, vec3(uv,i)).r;
        rho += f[i];
        p += f[i] * c[i];
        P += f[i] * dot(c[i] , c[i]);
    }

    //float v = pow(length(p) * push_constants.brightness, 1.3);

    float v = pow(P * push_constants.brightness, 1.3);

    vec3 rgb = hsv2rgb(vec3(0.0 + v * 0.1, 1.0-v, 0.99*v + 0.01));

    f_color = vec4(vec3(rgb), 1.0);
 
}