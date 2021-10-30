#version 450

layout (location=0) in vec2 v_pos;

layout(set=0, binding=0) uniform texture2D tex;
layout(set=0, binding=1) uniform sampler samp;

layout(location=0) out vec4 out_color;

void main() {
    vec2 t_pos = (vec2(-1.0, 1.0) - v_pos) * 0.5;
    out_color = vec4(texture(sampler2D(tex, samp), t_pos).rgb, 1.0);
}
