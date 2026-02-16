#version 330

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// fadeColor.r/g/b = tint color, fadeColor.a = override strength
// - 0..1   (e.g. vec4(1.0, 0.0, 0.0, 0.5))
uniform vec4 fadeColor;

// Output
out vec4 finalColor;

vec3 srgb_to_linear(vec3 c) {
    return pow(c, vec3(2.2));
}

vec3 linear_to_srgb(vec3 c) {
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
}

// vec4 normalizeRGBA(vec4 c) {
//     float maxComp = max(max(c.r, c.g), max(c.b, c.a));
//     if (maxComp > 1.0) {
//         return clamp(c / 255.0, 0.0, 1.0);
//     }
//     return clamp(c, 0.0, 1.0);
// }

void main() {
    vec4 sceneColor = texture(texture0, fragTexCoord);
    vec4 fade = fadeColor; // normalizeRGBA(fadeColor);

    float strength = fade.a;
    vec3 sceneLinear = srgb_to_linear(sceneColor.rgb);
    vec3 fadeLinear = srgb_to_linear(fade.rgb);
    vec3 rgb = linear_to_srgb(mix(sceneLinear, fadeLinear, strength));
    float alpha = mix(sceneColor.a, 1.0, strength);

    finalColor = vec4(rgb, alpha);
}
