#version 450

const vec2 positions[6] = vec2[6](
    vec2(-1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2( 1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2(-1.0, -1.0),
    vec2(-1.0,  1.0)
);

layout (location=0) out vec2 v_pos;

void main() {
    v_pos = positions[gl_VertexIndex];
    gl_Position = vec4(v_pos, 0.0, 1.0);
}
