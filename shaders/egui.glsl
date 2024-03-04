#shader vertex

#version 450

// epaint::Vertex
layout (location = 0) in vec2 v_pos;
layout (location = 1) in vec4 v_color;
layout (location = 2) in vec2 v_uv;

layout (location = 0) out vec4 o_color;
layout (location = 1) out vec2 o_uv;

layout (push_constant) uniform PushConstants {
    vec2 screen_size;
} push_constants;

void main() {
    gl_Position = vec4(
        2.0 * v_pos.x / push_constants.screen_size.x - 1.0,
        2.0 * v_pos.y / push_constants.screen_size.y - 1.0,
        0.0,
        1.0
    );
    o_color = v_color;
    o_uv = v_uv;
}

#shader fragment

#version 450

layout (location = 0) in vec4 i_color;
layout (location = 1) in vec2 i_uv;

layout (location = 0) out vec4 f_color;

layout (set = 0, binding = 0) uniform sampler2D font_texture;

void main() {
    f_color = i_color * texture(font_texture, i_uv);
}
