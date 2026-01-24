#version 330

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// Standard uniforms provided by engine
uniform float uTime;
uniform vec2 uResolution;

// Custom uniforms (set from Lua)
uniform float amplitude;
uniform float length;
uniform float speed;

// Output
out vec4 finalColor;

void main() {
    // Create a subtle wave distortion effect
    vec2 uv = fragTexCoord;

    // Add time-based wave distortion
    float wave = sin(uv.y * length + uTime * speed) * amplitude;
    uv.x += wave;

    vec4 color = texture(texture0, uv);
    finalColor = color;
}
