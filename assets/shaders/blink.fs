#version 330

// Blink Shader (entity shader)
//
// Alternates a sprite between its normal tint color and a configurable
// blink color with a hard cut. The sprite shape (texture alpha) is
// always preserved.
//
// User uniforms:
//   colBlink   (vec4)  - Blink color in RGBA (0.0-1.0).
//                        RGB overrides the sprite color; A controls opacity.
//   uCycleTime (float) - Total cycle duration in seconds.
//   uBlinkPct  (float) - Fraction of the cycle spent in the blink state (0.0-1.0).
//
// Usage (Lua):
//   -- Load in on_setup:
//   engine.load_shader("blink", nil, "shaders/blink.fs")
//
//   -- Apply to an entity (white flash, 1s cycle, 50% blink):
//   :with_shader("blink", {
//       colBlink   = {1.0, 1.0, 1.0, 1.0},
//       uCycleTime = 1.0,
//       uBlinkPct  = 0.5,
//   })
//
//   -- Or update at runtime:
//   engine.entity_shader_set_vec4(id, "colBlink", 0.0, 0.0, 1.0, 0.5)
//   engine.entity_shader_set_float(id, "uCycleTime", 3.0)
//   engine.entity_shader_set_float(id, "uBlinkPct", 0.33)

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// Raylib tint color (set automatically from draw call tint parameter)
uniform vec4 colDiffuse;

// User uniforms
uniform vec4 colBlink;       // Blink color (RGBA, 0.0-1.0)
uniform float uCycleTime;    // Total cycle duration in seconds
uniform float uBlinkPct;     // Fraction of cycle spent in blink state (0.0-1.0)

// Standard uniforms (provided automatically by the engine)
uniform float uTime;

// Output
out vec4 finalColor;

void main() {
    vec4 texelColor = texture(texture0, fragTexCoord);

    // Position within the current cycle (0.0 to 1.0)
    float phase = fract(uTime / max(uCycleTime, 0.001));

    // Blink occupies the tail end of the cycle
    bool blinking = phase >= (1.0 - uBlinkPct);

    if (blinking) {
        // Blink: override color, preserve sprite shape via texture alpha
        finalColor = vec4(colBlink.rgb, texelColor.a * colBlink.a);
    } else {
        // Normal: standard raylib tint
        finalColor = texelColor * colDiffuse * fragColor;
    }
}
