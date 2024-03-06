#shader vertex

#version 450

layout (location = 0) in vec3 v_position;
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec3 v_color;
layout (location = 3) in vec2 v_texcoord;

layout (location = 0) out vec3 o_color;

// Camera uniform buffer block
layout(set = 0, binding = 1) uniform Camera {
    mat4 viewproj;
    float near;
    float far;
} camera;

void main() {
    gl_Position = camera.viewproj * vec4(v_position, 1.0f);
    o_color = v_color;
}

#shader fragment

#version 450

layout (location = 0) in vec3 i_color;
layout (location = 0) out vec4 f_color;

void main() {
    f_color = vec4(i_color, 1.0f);
}
