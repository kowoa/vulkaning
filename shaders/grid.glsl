#shader vertex

#version 450

layout (location = 0) in vec3 v_position;
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec3 v_color;
layout (location = 3) in vec2 v_texcoord;

layout (location = 0) out vec3 near_world_point;
layout (location = 1) out vec3 far_world_point;

layout(set = 0, binding = 0) uniform GpuSceneData {
    mat4 viewproj;
    float near;
    float far;
    vec4 ambient_color;
    vec4 sunlight_direction;
    vec4 sunlight_color;
} scene;

vec3 clip_to_world(vec3 clip_pos) {
    mat4 viewproj_inv = inverse(scene.viewproj);
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

layout(set = 0, binding = 0) uniform GpuSceneData {
    mat4 viewproj;
    float near;
    float far;
    vec4 ambient_color;
    vec4 sunlight_direction;
    vec4 sunlight_color;
} scene;

// frag_pos_world is the position of the fragment in world space
// lines_per_unit is the number of grid lines per world space unit
// lines_per_unit determines the spacing between grid lines
vec4 grid_color(vec3 frag_pos_world, float lines_per_unit, float line_weight) {
    vec2 coord = frag_pos_world.xz * lines_per_unit;
    vec2 derivative = fwidth(coord);
    // grid.x represents the proximity from the fragment to the nearest z grid line
    // grid.y represents the proximity from the fragment to the nearest x grid line
    // A proximity of 0 means the fragment is on the grid line
    // A proximity of 1 means the fragment is exactly in the middle of two grid lines
    vec2 grid = abs(fract(coord - 0.5) - 0.5) / derivative;
    float line = min(grid.x, grid.y);
    vec4 color = vec4(vec3(0.05), 1.0 - min(line, 1.0));

    // Color the x-axis blue
    float minz = min(derivative.y, 1);
    if (abs(frag_pos_world.z) < minz) {
        color.b = 1.0;
    }

    // Color the z-axis red
    float minx = min(derivative.x, 1);
    if (abs(frag_pos_world.x) < minx) {
        color.r = 1.0;
    }

    return color * line_weight;
}

// Compute the depth of the fragment from the camera's perspective in clip space
float clip_pos_depth(vec3 world_pos) {
    vec4 clip_pos = scene.viewproj * vec4(world_pos.xyz, 1.0);
    return (clip_pos.z / clip_pos.w);
}

// Linear depth is the depth that is linearly interpolated between the near and far planes
float clip_pos_linear_depth(vec3 world_pos) {
    // Transform world pos to clip space
    vec4 clip_pos = scene.viewproj * vec4(world_pos.xyz, 1.0);
    // Calculate clip space depth and scale to range [-1, 1]
    float clip_depth = (clip_pos.z / clip_pos.w) * 2.0 - 1.0;
    // Get linear depth value between near and far
    float linear_depth = (2.0 * scene.near * scene.far) / (scene.far + scene.near - clip_depth * (scene.far - scene.near));
    return linear_depth / scene.far; // Normalize
}

void main() {
    float t = -near_world_point.y / (far_world_point.y - near_world_point.y);
    // Lerp between near and far points to get the world position of the fragment
    vec3 frag_pos_world = near_world_point + t * (far_world_point - near_world_point);

    gl_FragDepth = clip_pos_depth(frag_pos_world);

    float linear_depth = clip_pos_linear_depth(frag_pos_world);
    float fading = max(0, (0.5 - linear_depth));

    // If t > 0, the fragment is on the XZ plane and therefore on the grid
    vec4 grid = grid_color(frag_pos_world, 1.0, 1.0) + grid_color(frag_pos_world, 0.1, 4.0);
    f_color = grid * float(t > 0);
    f_color.a *= fading;
}

