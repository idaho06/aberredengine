#version 330

// Example of usage in the lua scripts:
// function on_setup()
//     engine.load_shader("crt", nil, "./assets/shaders/crt.fs")
// end
// 
// function on_switch_scene(scene)
//     engine.post_process_shader("crt")
//     engine.post_process_set_float("uCurvature", 0.8)
//     engine.post_process_set_float("uScanline", 0.9)
//     engine.post_process_set_float("uVignette", 0.7)
//     engine.post_process_set_float("uFlicker", 0.4)
// end

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// Standard uniforms provided by engine
uniform vec2 uResolution;
uniform float uTime;

// Custom uniforms (set from Lua)
uniform float uCurvature;  // 0..1, barrel distortion strength
uniform float uScanline;   // 0..1, scanline strength
uniform float uVignette;   // 0..1, vignette strength
uniform float uFlicker;    // 0..1, flicker strength

// Output
out vec4 finalColor;

void main() {
    vec2 resolution = max(uResolution, vec2(1.0));
    vec2 uv = fragTexCoord;

    float curvature = clamp(uCurvature, 0.0, 1.0);
    float scanlineStrength = clamp(uScanline, 0.0, 1.0);
    float vignetteStrength = clamp(uVignette, 0.0, 1.0);
    float flickerStrength = clamp(uFlicker, 0.0, 1.0);

    // Slight barrel distortion (CRT curvature)
    vec2 centered = uv * 2.0 - 1.0;
    float r2 = dot(centered, centered);
    vec2 curved = centered * (1.0 + (0.08 * curvature) * r2);
    vec2 uvCurved = curved * 0.5 + 0.5;

    if (uvCurved.x < 0.0 || uvCurved.x > 1.0 || uvCurved.y < 0.0 || uvCurved.y > 1.0) {
        finalColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    vec4 color = texture(texture0, uvCurved) * fragColor;

    // Horizontal scanlines
    float scanline = 1.0 - scanlineStrength * 0.1 + scanlineStrength * 0.1 * cos(uvCurved.y * resolution.y * 3.14159);
    color.rgb *= scanline;

    // Vignette
    float vignette = 1.0 - smoothstep(0.4, 1.0, length(centered));
    color.rgb *= mix(1.0, vignette, vignetteStrength);

    // Subtle flicker
    float flicker = 1.0 - 0.02 * flickerStrength + 0.02 * flickerStrength * sin(uTime * 60.0);
    color.rgb *= flicker;

    finalColor = color;
}
