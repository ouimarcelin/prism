// Shared by every effect: one full-screen pass reading the image so far.
//
// Colors are premultiplied by alpha and kept in display space, see
// docs/decisions/0004-espace-de-mixage.md.

struct Params {
    // Settings of the effect, in the order of the catalog.
    values: array<vec4<f32>, 2>,
    // Size of the image in pixels.
    size: vec2<f32>,
    // Seconds since the engine started.
    time: f32,
    // Beat position of the tempo clock.
    beat: f32,
    // Line height of the text mask read as `source`, in pixels (text
    // generator only).
    mask_line: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle covering the whole target; no vertex buffer.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: VertexOutput;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

const PI: f32 = 3.14159265358979;

// Weights of red, green and blue in the brightness of a color (BT.709).
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

// A pseudo-random value in 0..1, the same for the same input.
fn hash(value: f32) -> f32 {
    return fract(sin(value * 12.9898) * 43758.5453);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3<f32>(hsv.x) + k) * 6.0 - vec3<f32>(3.0));
    return hsv.z * mix(vec3<f32>(1.0), clamp(p - vec3<f32>(1.0), vec3<f32>(0.0), vec3<f32>(1.0)), hsv.y);
}

fn param(index: u32) -> f32 {
    return params.values[index / 4u][index % 4u];
}

// The image at `uv`; transparent outside of it.
fn sample(uv: vec2<f32>) -> vec4<f32> {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    return textureSampleLevel(source, source_sampler, uv, 0.0);
}

// The image at `uv`, its edge repeated outside of it.
fn sample_clamped(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source, source_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
}

fn unpremultiply(color: vec4<f32>) -> vec3<f32> {
    if color.a <= 0.0 {
        return vec3<f32>(0.0);
    }
    return color.rgb / color.a;
}

fn premultiply(rgb: vec3<f32>, alpha: f32) -> vec4<f32> {
    return vec4<f32>(rgb * alpha, alpha);
}

// From a point of the image, in pixels from its center, to texture
// coordinates.
fn to_uv(pixels: vec2<f32>) -> vec2<f32> {
    return pixels / params.size + vec2<f32>(0.5);
}

// From texture coordinates to pixels from the center of the image.
fn from_uv(uv: vec2<f32>) -> vec2<f32> {
    return (uv - vec2<f32>(0.5)) * params.size;
}

fn rotate(point: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * point.x - s * point.y, s * point.x + c * point.y);
}
