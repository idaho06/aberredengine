#version 330

//=============================================================================
// CRT STYLED SCAN-LINE SHADER (GLSL 3.3)
//=============================================================================
// Original: PUBLIC DOMAIN by Timothy Lottes
// Converted for Aberred Engine post-processing pipeline
//
// USAGE IN LUA:
// -------------
// 1. Load the shader in on_setup:
//    engine.load_shader("crt", nil, "shaders/crt2.fs")
//
// 2. Activate in on_switch_scene or on_update:
//    engine.post_process_shader({"crt"})
//
// 3. Configure uniforms (optional - defaults are sensible):
//    engine.post_process_set_float("uResDivisor", 4.0)  -- CRT pixel size (1.0=sharp, 6.0=chunky)
//    engine.post_process_set_int("uMaskStyle", 1)       -- Shadow mask style (0-3)
//
// UNIFORMS:
// ---------
// Standard (set automatically by engine):
//   uResolution   vec2   Game render resolution
//   uTime         float  Elapsed time in seconds
//   uDeltaTime    float  Frame delta time
//   uFrame        int    Frame counter
//
// Custom:
//   uResDivisor   float  Resolution divisor for CRT emulation (default: 4.0)
//                        Lower = sharper pixels, Higher = chunkier CRT look
//                        Recommended range: 1.0 - 8.0
//
//   uMaskStyle    int    Shadow mask style (default: 1)
//                        0 = Compressed TV (rotated mask, less chromatic aberration)
//                        1 = Aperture-grille (vertical RGB stripes)
//                        2 = Stretched VGA (diagonal pattern)
//                        3 = VGA (classic VGA shadow mask)
//
// HARDCODED PARAMETERS (edit shader to change):
// ---------------------------------------------
//   hardScan      -10.0  Scanline hardness (-8=soft, -16=medium)
//   hardPix       -4.0   Pixel hardness (-2=soft, -4=hard)
//   hardBloomScan -2.0   Vertical bloom hardness
//   hardBloomPix  -1.5   Horizontal bloom hardness
//   bloomAmount   1/16   Bloom intensity (0=none, 1/16=subtle, 1/1=full)
//   warp          vec2   Screen curvature (0=flat, 1/8=extreme)
//   maskDark      0.5    Shadow mask dark level
//   maskLight     1.5    Shadow mask light level
//=============================================================================

// Input from vertex shader
in vec2 fragTexCoord;

// Output
out vec4 finalColor;

// Standard engine uniforms
uniform sampler2D texture0;
uniform vec2 uResolution;

// Custom uniforms
uniform float uResDivisor = 4.0;
uniform int uMaskStyle = 1;

// Hardcoded parameters
float hardScan = -10.0;
float hardPix = -4.0;
float hardBloomScan = -2.0;
float hardBloomPix = -1.5;
float bloomAmount = 1.0 / 16.0;
vec2 warp = vec2(1.0 / 64.0, 1.0 / 24.0);
float maskDark = 0.5;
float maskLight = 1.5;

//------------------------------------------------------------------------

// sRGB to Linear
float ToLinear1(float c) {
    return (c <= 0.04045) ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}
vec3 ToLinear(vec3 c) {
    return vec3(ToLinear1(c.r), ToLinear1(c.g), ToLinear1(c.b));
}

// Linear to sRGB
float ToSrgb1(float c) {
    return (c < 0.0031308 ? c * 12.92 : 1.055 * pow(c, 0.41666) - 0.055);
}
vec3 ToSrgb(vec3 c) {
    return vec3(ToSrgb1(c.r), ToSrgb1(c.g), ToSrgb1(c.b));
}

// Emulated resolution
vec2 Res() {
    return uResolution.xy / uResDivisor;
}

// Nearest emulated sample given floating point position and texel offset.
// Also zero's off screen.
vec3 Fetch(vec2 pos, vec2 off) {
    vec2 res = Res();
    pos = floor(pos * res + off) / res;
    if (max(abs(pos.x - 0.5), abs(pos.y - 0.5)) > 0.5) return vec3(0.0);
    return ToLinear(texture(texture0, pos.xy).rgb);
}

// Distance in emulated pixels to nearest texel.
vec2 Dist(vec2 pos) {
    vec2 res = Res();
    pos = pos * res;
    return -((pos - floor(pos)) - vec2(0.5));
}

// 1D Gaussian.
float Gaus(float pos, float scale) {
    return exp2(scale * pos * pos);
}

// 3-tap Gaussian filter along horz line.
vec3 Horz3(vec2 pos, float off) {
    vec3 b = Fetch(pos, vec2(-1.0, off));
    vec3 c = Fetch(pos, vec2(0.0, off));
    vec3 d = Fetch(pos, vec2(1.0, off));
    float dst = Dist(pos).x;
    float scale = hardPix;
    float wb = Gaus(dst - 1.0, scale);
    float wc = Gaus(dst + 0.0, scale);
    float wd = Gaus(dst + 1.0, scale);
    return (b * wb + c * wc + d * wd) / (wb + wc + wd);
}

// 5-tap Gaussian filter along horz line.
vec3 Horz5(vec2 pos, float off) {
    vec3 a = Fetch(pos, vec2(-2.0, off));
    vec3 b = Fetch(pos, vec2(-1.0, off));
    vec3 c = Fetch(pos, vec2(0.0, off));
    vec3 d = Fetch(pos, vec2(1.0, off));
    vec3 e = Fetch(pos, vec2(2.0, off));
    float dst = Dist(pos).x;
    float scale = hardPix;
    float wa = Gaus(dst - 2.0, scale);
    float wb = Gaus(dst - 1.0, scale);
    float wc = Gaus(dst + 0.0, scale);
    float wd = Gaus(dst + 1.0, scale);
    float we = Gaus(dst + 2.0, scale);
    return (a * wa + b * wb + c * wc + d * wd + e * we) / (wa + wb + wc + wd + we);
}

// 7-tap Gaussian filter along horz line.
vec3 Horz7(vec2 pos, float off) {
    vec3 a = Fetch(pos, vec2(-3.0, off));
    vec3 b = Fetch(pos, vec2(-2.0, off));
    vec3 c = Fetch(pos, vec2(-1.0, off));
    vec3 d = Fetch(pos, vec2(0.0, off));
    vec3 e = Fetch(pos, vec2(1.0, off));
    vec3 f = Fetch(pos, vec2(2.0, off));
    vec3 g = Fetch(pos, vec2(3.0, off));
    float dst = Dist(pos).x;
    float scale = hardBloomPix;
    float wa = Gaus(dst - 3.0, scale);
    float wb = Gaus(dst - 2.0, scale);
    float wc = Gaus(dst - 1.0, scale);
    float wd = Gaus(dst + 0.0, scale);
    float we = Gaus(dst + 1.0, scale);
    float wf = Gaus(dst + 2.0, scale);
    float wg = Gaus(dst + 3.0, scale);
    return (a * wa + b * wb + c * wc + d * wd + e * we + f * wf + g * wg) / (wa + wb + wc + wd + we + wf + wg);
}

// Return scanline weight.
float Scan(vec2 pos, float off) {
    float dst = Dist(pos).y;
    return Gaus(dst + off, hardScan);
}

// Return scanline weight for bloom.
float BloomScan(vec2 pos, float off) {
    float dst = Dist(pos).y;
    return Gaus(dst + off, hardBloomScan);
}

// Allow nearest three lines to affect pixel.
vec3 Tri(vec2 pos) {
    vec3 a = Horz3(pos, -1.0);
    vec3 b = Horz5(pos, 0.0);
    vec3 c = Horz3(pos, 1.0);
    float wa = Scan(pos, -1.0);
    float wb = Scan(pos, 0.0);
    float wc = Scan(pos, 1.0);
    return a * wa + b * wb + c * wc;
}

// Small bloom.
vec3 Bloom(vec2 pos) {
    vec3 a = Horz5(pos, -2.0);
    vec3 b = Horz7(pos, -1.0);
    vec3 c = Horz7(pos, 0.0);
    vec3 d = Horz7(pos, 1.0);
    vec3 e = Horz5(pos, 2.0);
    float wa = BloomScan(pos, -2.0);
    float wb = BloomScan(pos, -1.0);
    float wc = BloomScan(pos, 0.0);
    float wd = BloomScan(pos, 1.0);
    float we = BloomScan(pos, 2.0);
    return a * wa + b * wb + c * wc + d * wd + e * we;
}

// Distortion of scanlines, and end of screen alpha.
vec2 Warp(vec2 pos) {
    pos = pos * 2.0 - 1.0;
    pos *= vec2(1.0 + (pos.y * pos.y) * warp.x, 1.0 + (pos.x * pos.x) * warp.y);
    return pos * 0.5 + 0.5;
}

// Shadow mask - Style 0: Compressed TV (rotated mask)
vec3 MaskCompressedTV(vec2 pos) {
    float line = maskLight;
    float odd = 0.0;
    if (fract(pos.x / 6.0) < 0.5) odd = 1.0;
    if (fract((pos.y + odd) / 2.0) < 0.5) line = maskDark;
    pos.x = fract(pos.x / 3.0);
    vec3 mask = vec3(maskDark);
    if (pos.x < 0.333) mask.r = maskLight;
    else if (pos.x < 0.666) mask.g = maskLight;
    else mask.b = maskLight;
    mask *= line;
    return mask;
}

// Shadow mask - Style 1: Aperture-grille (vertical RGB stripes)
vec3 MaskApertureGrille(vec2 pos) {
    pos.x = fract(pos.x / 3.0);
    vec3 mask = vec3(maskDark);
    if (pos.x < 0.333) mask.r = maskLight;
    else if (pos.x < 0.666) mask.g = maskLight;
    else mask.b = maskLight;
    return mask;
}

// Shadow mask - Style 2: Stretched VGA (diagonal pattern)
vec3 MaskStretchedVGA(vec2 pos) {
    pos.x += pos.y * 3.0;
    vec3 mask = vec3(maskDark);
    pos.x = fract(pos.x / 6.0);
    if (pos.x < 0.333) mask.r = maskLight;
    else if (pos.x < 0.666) mask.g = maskLight;
    else mask.b = maskLight;
    return mask;
}

// Shadow mask - Style 3: VGA (classic)
vec3 MaskVGA(vec2 pos) {
    pos.xy = floor(pos.xy * vec2(1.0, 0.5));
    pos.x += pos.y * 3.0;
    vec3 mask = vec3(maskDark);
    pos.x = fract(pos.x / 6.0);
    if (pos.x < 0.333) mask.r = maskLight;
    else if (pos.x < 0.666) mask.g = maskLight;
    else mask.b = maskLight;
    return mask;
}

// Select mask based on uniform
vec3 Mask(vec2 pos) {
    if (uMaskStyle == 0) return MaskCompressedTV(pos);
    if (uMaskStyle == 2) return MaskStretchedVGA(pos);
    if (uMaskStyle == 3) return MaskVGA(pos);
    return MaskApertureGrille(pos); // Default: style 1
}

// Entry point
void main() {
    // Use fragTexCoord (0-1 UV space) instead of gl_FragCoord for proper scaling
    vec2 uv = fragTexCoord;
    vec2 pos = Warp(uv);

    // Derive pixel coordinates for mask from UV and resolution
    vec2 pixelCoord = uv * uResolution.xy;

    vec3 color = Tri(pos) * Mask(pixelCoord);

    // Additive bloom
    color += Bloom(pos) * bloomAmount;

    finalColor = vec4(ToSrgb(color), 1.0);
}
