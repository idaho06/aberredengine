#version 330

// Parallax Scroll Shader (entity shader)
//
// Keeps the entity quad fixed in screen space while scrolling the sampled
// texture inside it. The scroll offset is provided in pixel space so Lua can
// drive the effect directly from camera movement.
//
// User uniforms:
//   uScrollPx (vec2) - Texture scroll offset in pixels.
//
// Engine uniforms:
//   uSpriteSize (vec2) - Sprite width/height in pixels.

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture and tint
uniform sampler2D texture0;
uniform vec4 colDiffuse;

// Engine-provided entity uniform
uniform vec2 uSpriteSize;

// User-defined uniform
uniform vec2 uScrollPx;

// Output
out vec4 finalColor;

void main() {
    vec2 spriteSize = max(uSpriteSize, vec2(1.0));
    vec2 scrollUv = uScrollPx / spriteSize;
    vec2 uv = fract(fragTexCoord + scrollUv);

    vec4 texelColor = texture(texture0, uv);
    finalColor = texelColor * colDiffuse * fragColor;
}