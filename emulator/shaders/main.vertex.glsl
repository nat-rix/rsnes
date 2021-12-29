#version 450

layout (location=0) out vec2 v_pos;

void main() {
    v_pos = vec2(1.0, 1.0);
    if (gl_VertexIndex == 0 || gl_VertexIndex > 3)
        v_pos.x = -1.0;
    if ((gl_VertexIndex & 1) == 0)
        v_pos.y = -1.0;
    gl_Position = vec4(v_pos, 0.0, 1.0);
}
