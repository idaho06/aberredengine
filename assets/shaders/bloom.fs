#version 330

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// Standard uniforms provided by engine
uniform vec2 uResolution;

// Custom uniforms (set from Lua)
uniform float threshold; // 0..1, brightness threshold
uniform float intensity; // bloom strength
uniform float radius;    // blur radius in pixels

// Output
out vec4 finalColor;

float luminance(vec3 color) {
    return dot(color, vec3(0.2126, 0.7152, 0.0722));
}

vec3 bright_pass(vec3 color) {
    float lum = luminance(color);
    float knee = 0.1;
    float factor = smoothstep(threshold, threshold + knee, lum);
    return color * factor;
}

void main() {
    vec2 uv = fragTexCoord;
    vec2 texel = 1.0 / max(uResolution, vec2(1.0));
    vec2 r = texel * radius;

    vec4 base = texture(texture0, uv) * fragColor;

    vec3 c0 = bright_pass(texture(texture0, uv).rgb);
    vec3 c1 = bright_pass(texture(texture0, uv + vec2( r.x, 0.0)).rgb);
    vec3 c2 = bright_pass(texture(texture0, uv + vec2(-r.x, 0.0)).rgb);
    vec3 c3 = bright_pass(texture(texture0, uv + vec2(0.0,  r.y)).rgb);
    vec3 c4 = bright_pass(texture(texture0, uv + vec2(0.0, -r.y)).rgb);
    vec3 c5 = bright_pass(texture(texture0, uv + vec2( r.x,  r.y)).rgb);
    vec3 c6 = bright_pass(texture(texture0, uv + vec2(-r.x,  r.y)).rgb);
    vec3 c7 = bright_pass(texture(texture0, uv + vec2( r.x, -r.y)).rgb);
    vec3 c8 = bright_pass(texture(texture0, uv + vec2(-r.x, -r.y)).rgb);

    vec3 bloom =
        c0 * 0.20 +
        (c1 + c2 + c3 + c4) * 0.125 +
        (c5 + c6 + c7 + c8) * 0.075;

    vec3 color = base.rgb + bloom * intensity;
    finalColor = vec4(color, base.a);
}
