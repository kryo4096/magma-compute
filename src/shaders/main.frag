#version 460

layout(push_constant) uniform PushConstants {
    float brightness;
} push_constants;

layout(set=0, binding=0) uniform sampler2DArray tex;
layout(set=0, binding=1) uniform usampler2D type;

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



// HSL CODE FROM: https://github.com/Jam3/glsl-hsl2rgb:

/*The MIT License (MIT) Copyright (c) 2015 Jam3

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.*/

float hue2rgb(float f1, float f2, float hue) {
    if (hue < 0.0)
        hue += 1.0;
    else if (hue > 1.0)
        hue -= 1.0;
    float res;
    if ((6.0 * hue) < 1.0)
        res = f1 + (f2 - f1) * 6.0 * hue;
    else if ((2.0 * hue) < 1.0)
        res = f2;
    else if ((3.0 * hue) < 2.0)
        res = f1 + (f2 - f1) * ((2.0 / 3.0) - hue) * 6.0;
    else
        res = f1;
    return res;
}

vec3 hsl2rgb(vec3 hsl) {
    vec3 rgb;
    
    if (hsl.y == 0.0) {
        rgb = vec3(hsl.z); // Luminance
    } else {
        float f2;
        
        if (hsl.z < 0.5)
            f2 = hsl.z * (1.0 + hsl.y);
        else
            f2 = hsl.z + hsl.y - hsl.y * hsl.z;
            
        float f1 = 2.0 * hsl.z - f2;
        
        rgb.r = hue2rgb(f1, f2, hsl.x + (1.0/3.0));
        rgb.g = hue2rgb(f1, f2, hsl.x);
        rgb.b = hue2rgb(f1, f2, hsl.x - (1.0/3.0));
    }   
    return rgb;
}

vec3 hsl2rgb(float h, float s, float l) {
    return hsl2rgb(vec3(h, s, l));
}

// END HSL CODE



void main() {

    float f[9];

    float rho = 0;
    vec2 p = vec2(0);
    float P = 0;

    if(texture(type, uv).r == 0) {

        for(int i = 0; i < 9; i++) {
            f[i] = texture(tex, vec3(uv,i)).r;
            rho += f[i];
            p += f[i] * c[i];
            P += f[i] * dot(c[i] , c[i]);
        }

        float v = pow(length(p) * push_constants.brightness, 1.5);

        //float v = pow(P * push_constants.brightness, 1.3);

        vec3 rgb = hsl2rgb(vec3(0.0 + v * 0.2, 1.0, v));

        f_color = vec4(vec3(rgb), 1.0);
    } else {

        f_color = vec4(0.1,0.1,0.1,1.0);

    }

 
}