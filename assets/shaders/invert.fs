#version 330

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// Output
out vec4 finalColor;

void main() {
    vec4 color = texture(texture0, fragTexCoord);
    finalColor = vec4(1.0 - color.rgb, color.a);
}
