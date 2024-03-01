#shader vertex

#version 450

layout (location = 0) in vec3 v_position;
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec3 v_color;
layout (location = 3) in vec2 v_texcoord;
layout (location = 0) out vec3 near_point;
layout (location = 1) out vec3 far_point;

layout (set = 0, binding = 1) uniform CameraUniforms {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
} Camera;

layout (push_constant) uniform ModelUniforms {
    mat4 model;
} Model;

vec3 unproject_point(float x, float y, float z, mat4 view, mat4 proj) {
    mat4 view_inv = inverse(view);
    mat4 proj_inv = inverse(proj);
    vec4 unprojected_point = view_inv * proj_inv * vec4(x, y, z, 1.0);
    return unprojected_point.xyz / unprojected_point.w;
}

void main() {
    vec3 p = v_position;
    // Unproject the position on the near plane (z=0)
    near_point = unproject_point(p.x, p.y, 0.0, Camera.view, Camera.proj).xyz;
    // Unproject the position on the far plane (z=1)
    far_point = unproject_point(p.x, p.y, 1.0, Camera.view, Camera.proj).xyz;
    gl_Position = vec4(p, 1.0);
}

#shader fragment

#version 450

layout (location = 0) in vec3 near_point;
layout (location = 1) in vec3 far_point;
layout (location = 0) out vec4 f_color;

// Scene uniform buffer block
layout (set = 0, binding = 0) uniform GpuSceneData {
    vec4 fogColor; // w is the exponent
    vec4 fogDistances; // x for min, y for max, zw unused
    vec4 ambientColor;
    vec4 sunlightDirection; // w for sun power
    vec4 sunlightColor;
} sceneData;

void main() {
    float t = -near_point.y / (far_point.y - near_point.y);
    f_color = vec4(1.0, 0.0, 0.0, 1.0 * float(t > 0));
}

