#shader vertex

#version 460

layout (location = 0) in vec3 vPosition;
layout (location = 1) in vec3 vNormal;
layout (location = 2) in vec3 vColor;

layout (location = 0) out vec3 outColor;

// Camera uniform buffer block
layout(set = 0, binding = 0) uniform CameraBuffer {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
} cameraData;

// Object data and storage buffer block
struct GpuObjectData {
    mat4 model;
};
layout (std140, set = 1, binding = 0) readonly buffer ObjectBuffer {
    GpuObjectData objects[];
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
}


#shader fragment

#version 450

layout (location = 0) in vec3 inColor;
layout (location = 0) out vec4 outFragColor;

void main() {
    outFragColor = vec4(inColor, 1.0f);
}
