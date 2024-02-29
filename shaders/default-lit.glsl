#shader vertex

#version 460

layout (location = 0) in vec3 vPosition;
layout (location = 1) in vec3 vNormal;
layout (location = 2) in vec3 vColor;
layout (location = 3) in vec2 vTexcoord;
layout (location = 0) out vec3 outColor;
layout (location = 1) out vec2 outTexcoord;

// Camera uniform buffer block
layout(set = 0, binding = 1) uniform GpuCameraData {
    mat4 viewproj;
} cameraData;

// Object data and storage buffer block
struct GpuObjectData {
    mat4 model;
};
layout (std140, set = 1, binding = 0) readonly buffer ObjectBuffer {
    GpuObjectData objects[]; // Shader will scale this array because this is a storage buffer
} objectBuffer;

// Push constants block
layout(push_constant) uniform constants {
    vec4 data;
    mat4 render_matrix;
} PushConstants;

void main() {
    mat4 modelMat = objectBuffer.objects[gl_BaseInstance].model;
    mat4 transformMat = (cameraData.viewproj * modelMat);
    gl_Position = transformMat * vec4(vPosition, 1.0f);

    outColor = vColor;
    outTexcoord = vTexcoord;
}

#shader fragment

#version 450

layout (location = 0) in vec3 inColor;
layout (location = 1) in vec2 inTexcoord;
layout (location = 0) out vec4 fColor;

layout (set = 0, binding = 0) uniform GpuSceneData {
    vec4 fogColor; // w is the exponent
    vec4 fogDistances; // x for min, y for max, zw unused
    vec4 ambientColor;
    vec4 sunlightDirection; // w for sun power
    vec4 sunlightColor;
} sceneData;

void main() {
    fColor = vec4(inColor + sceneData.ambientColor.xyz, 1.0f);
    //fColor = vec4(inTexcoord.x, inTexcoord.y, 0.5f, 1.0f);
}
