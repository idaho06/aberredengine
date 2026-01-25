#version 330

// Input from vertex shader
in vec2 fragTexCoord;
in vec4 fragColor;

// Input texture
uniform sampler2D texture0;

// User uniforms
uniform float uThickness;      // Outline thickness in pixels (default: 1.0)
uniform vec4 uColor;           // Outline color (RGBA)

// Standard uniforms (provided automatically)
uniform vec2 uSpriteSize;      // Sprite dimensions in pixels

// Output
out vec4 finalColor;

void main() {
    // Get the current pixel color
    vec4 texColor = texture(texture0, fragTexCoord);

    // Calculate pixel size in UV space (use full texture size, not sprite size)
    vec2 texSize = vec2(textureSize(texture0, 0));
    vec2 pixelSize = 1.0 / texSize;
    float thickness = max(uThickness, 1.0);

    // Sample neighbors to detect edges
    float maxAlpha = 0.0;

    // Sample in a square pattern around the pixel
    for (float x = -thickness; x <= thickness; x += 1.0) {
        for (float y = -thickness; y <= thickness; y += 1.0) {
            // Skip the center pixel
            if (x == 0.0 && y == 0.0) continue;

            // Only sample within the circular distance (optional, for rounder outlines)
            if (length(vec2(x, y)) > thickness) continue;

            vec2 offset = vec2(x, y) * pixelSize;
            vec2 sampleCoord = fragTexCoord + offset;

            // Clamp to valid UV range
            sampleCoord = clamp(sampleCoord, vec2(0.0), vec2(1.0));

            float neighborAlpha = texture(texture0, sampleCoord).a;
            maxAlpha = max(maxAlpha, neighborAlpha);
        }
    }

    // If current pixel is transparent but has an opaque neighbor, draw outline
    if (texColor.a < 0.1 && maxAlpha > 0.1) {
        finalColor = uColor;
    } else {
        // Draw original texture
        finalColor = texColor * fragColor;
    }
}
