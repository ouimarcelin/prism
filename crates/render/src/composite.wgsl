// Composites one layer over the result of the layers below it.
//
// Colors are premultiplied by alpha and kept in display (gamma-encoded) space,
// see docs/decisions/0004-espace-de-mixage.md. The blending formulas must stay
// identical to `reference.rs`.

struct LayerParams {
    opacity: f32,
    blend_mode: u32,
    // Transition from the previous clip: 1.0 when there is none.
    progress: f32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: LayerParams;
@group(0) @binding(1) var below_texture: texture_2d<f32>;
@group(0) @binding(2) var layer_texture: texture_2d<f32>;
@group(0) @binding(3) var layer_sampler: sampler;
@group(0) @binding(4) var previous_texture: texture_2d<f32>;

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

const BLEND_NORMAL: u32 = 0u;
const BLEND_ADD: u32 = 1u;
const BLEND_MULTIPLY: u32 = 2u;
const BLEND_SCREEN: u32 = 3u;

fn unpremultiply(color: vec4<f32>) -> vec3<f32> {
    if color.a <= 0.0 {
        return vec3<f32>(0.0);
    }
    return color.rgb / color.a;
}

// Blend function B(cb, cs) on straight (non-premultiplied) colors.
fn blend(cb: vec3<f32>, cs: vec3<f32>, mode: u32) -> vec3<f32> {
    switch mode {
        case BLEND_ADD: {
            return cb + cs;
        }
        case BLEND_MULTIPLY: {
            return cb * cs;
        }
        case BLEND_SCREEN: {
            return cb + cs - cb * cs;
        }
        default: {
            return cs;
        }
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let below = textureLoad(below_texture, vec2<i32>(in.position.xy), 0);
    let current = textureSample(layer_texture, layer_sampler, in.uv);
    let previous = textureSample(previous_texture, layer_sampler, in.uv);
    let layer = mix(previous, current, params.progress);

    let alpha_s = layer.a * params.opacity;
    let alpha_b = below.a;
    let cs = unpremultiply(layer);
    let cb = unpremultiply(below);

    let color = alpha_s * (1.0 - alpha_b) * cs
        + alpha_s * alpha_b * blend(cb, cs, params.blend_mode)
        + (1.0 - alpha_s) * below.rgb;
    let alpha = alpha_s + alpha_b * (1.0 - alpha_s);
    return vec4<f32>(color, alpha);
}
