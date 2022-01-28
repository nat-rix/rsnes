#version 450

layout (location=0) out vec2 v_pos;

layout(std140, set=0, binding=2) uniform ScreenInfo {
    ivec2 screen_size;
    ivec2 texture_size;
} info;

void main() {
    v_pos = vec2(1.0, 1.0);
    if (gl_VertexIndex == 0 || gl_VertexIndex > 3)
        v_pos.x = -1.0;
    if ((gl_VertexIndex & 1) == 0)
        v_pos.y = -1.0;

    float scr_prop = float(info.screen_size.x) / float(info.screen_size.y);
    float tex_prop = float(info.texture_size.x) / float(info.texture_size.y);

    gl_Position = vec4(v_pos, 0.0, 1.0);

    if (tex_prop > scr_prop) {
        // make black bars top and bottom
        gl_Position.y *= scr_prop / tex_prop;
    } else if (tex_prop < scr_prop) {
        // make black bars left and right
        gl_Position.x *= tex_prop / scr_prop;
    }
}
