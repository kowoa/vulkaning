#shader vertex

#version 450

layout (location = 0) in vec3 vPosition;
layout (location = 1) in vec3 vNormal;
layout (location = 2) in vec3 vColor;

layout (location = 0) out vec3 outColor;

void main() {
    gl_Position = vec4(vPosition, 1.0f);
    outColor = vColor;
}


#shader fragment

#version 450

layout (location = 0) in vec3 inColor;
layout (location = 0) out vec4 outFragColor;

void main() {
    outFragColor = vec4(inColor, 1.0f);
}
