#shader vertex

#version 450

layout (location = 0) in vec3 v_position;
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec3 v_color;
layout (location = 3) in vec2 v_texcoord;
layout (location = 0) out vec3 out_color;
layout (location = 1) out vec2 out_texcoord;

layout (binding = 1) uniform Camera {
    mat4 viewproj;
};

layout (push_constant) uniform Model {
    mat4 model;
};

void main() {
    out_color = v_color;
    out_texcoord = v_texcoord;
    gl_Position = viewproj * model * vec4(v_position, 1.0);
}

#shader fragment

#version 450

layout (location = 0) in vec3 in_color;
layout (location = 1) in vec2 in_texcoord;
layout (location = 0) out vec4 f_color;

void main() {
    f_color = vec4(in_color, 1.0);
}

