#shader vertex

#version 450

void main() {
    const vec3 positions[3] = vec3[3](
        vec3(1.f, 1.f, 0.0f),
        vec3(-1.f, 1.f, 0.0f),
        vec3(0.f, -1.f, 0.0f)
    );

    gl_Position = vec4(positions[gl_VertexIndex], 1.0f);
}


#shader fragment

#version 450

layout (location = 0) out vec4 outFragColor;

void main() {
    outFragColor = vec4(1.f, 0.f, 0.f, 1.0f);
}
