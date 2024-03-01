#shader vertex

#version 450

layout (location = 0) in vec3 v_position;
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec3 v_color;
layout (location = 3) in vec2 v_texcoord;

layout (location = 0) out vec3 near_world_point;
layout (location = 1) out vec3 far_world_point;

layout (set = 0, binding = 1) uniform CameraUniforms {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
} Camera;

layout (push_constant) uniform ModelUniforms {
    mat4 model;
} Model;

vec3 clip_to_world(vec3 clip_pos) {
    mat4 viewproj_inv = inverse(Camera.viewproj);
    vec4 world_pos = viewproj_inv * vec4(clip_pos, 1.0);
    world_pos /= world_pos.w; // Undo perspective projection
    return world_pos.xyz;
}

void main() {
    vec3 clip_pos = v_position;
    // Get the world space position on the near plane
    near_world_point = clip_to_world(vec3(clip_pos.xy, 0.0));
    // Get the world space position on the far plane
    far_world_point = clip_to_world(vec3(clip_pos.xy, 1.0));
    gl_Position = vec4(clip_pos, 1.0);
}

#shader fragment

#version 450

layout (location = 0) in vec3 near_world_point;
layout (location = 1) in vec3 far_world_point;
layout (location = 0) out vec4 f_color;

// Scene uniform buffer block
layout (set = 0, binding = 0) uniform GpuSceneData {
    vec4 fogColor; // w is the exponent
    vec4 fogDistances; // x for min, y for max, zw unused
    vec4 ambientColor;
    vec4 sunlightDirection; // w for sun power
    vec4 sunlightColor;
} sceneData;

// frag_pos_world is the position of the fragment in world space
// scale determines the distance between the grid lines
vec4 grid_color(vec3 frag_pos_world, float scale) {
    vec2 coord = frag_pos_world.xz * scale;
    vec2 derivative = fwidth(coord);
    // grid.x represents the proxity from the fragment to the nearest z grid line
    // grid.y represents the proxity from the fragment to the nearest x grid line
    // A proximity of 0 means the fragment is on the grid line
    // A proximity of 1 means the fragment is exactly in the middle of two grid lines
    vec2 grid = abs(fract(coord - 0.5) - 0.5) / derivative;
    float line = min(grid.x, grid.y);
    vec4 color = vec4(0.2, 0.2, 0.2, 1.0 - min(line, 1.0));

    // Color the x-axis blue
    float minz = min(derivative.y, 1);
    if (abs(frag_pos_world.z) < 0.1 * minz) {
        color.b = 1.0;
    }

    // Color the z-axis red
    float minx = min(derivative.x, 1);
    if (abs(frag_pos_world.x) < 0.1 * minx) {
        color.r = 1.0;
    }

    return color;
}

void main() {
    float t = -near_world_point.y / (far_world_point.y - near_world_point.y);
    // Lerp between near and far points to get the world position of the fragment
    vec3 frag_pos_world = near_world_point + t * (far_world_point - near_world_point);
    // If t > 0, the fragment is on the XZ plane and therefore on the grid
    f_color = grid_color(frag_pos_world, 10.0) * float(t > 0);
}

