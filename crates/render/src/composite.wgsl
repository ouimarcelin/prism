// Composites up to four layers over the result of the layers below them, in
// one pass: the mix stays in 32-bit registers from one layer to the next.
//
// Colors are premultiplied by alpha and kept in display (gamma-encoded) space,
// see docs/decisions/0004-espace-de-mixage.md. The blending formulas must stay
// identical to `reference.rs`.

struct Layer {
    opacity: f32,
    blend_mode: u32,
    // Transition from the previous clip: 1.0 when there is none.
    progress: f32,
    _pad: u32,
}

struct Batch {
    layers: array<Layer, 4>,
    // Layers of this pass, from the first.
    count: u32,
    // 1 when the pass starts from opaque black instead of `below_texture`.
    over_black: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<uniform> batch: Batch;
@group(0) @binding(1) var below_texture: texture_2d<f32>;
@group(0) @binding(2) var layer_sampler: sampler;
@group(0) @binding(3) var layer0: texture_2d<f32>;
@group(0) @binding(4) var previous0: texture_2d<f32>;
@group(0) @binding(5) var layer1: texture_2d<f32>;
@group(0) @binding(6) var previous1: texture_2d<f32>;
@group(0) @binding(7) var layer2: texture_2d<f32>;
@group(0) @binding(8) var previous2: texture_2d<f32>;
@group(0) @binding(9) var layer3: texture_2d<f32>;
@group(0) @binding(10) var previous3: texture_2d<f32>;

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
const BLEND_LIGHTEN: u32 = 4u;
const BLEND_DARKEN: u32 = 5u;
const BLEND_DIFFERENCE: u32 = 6u;
const BLEND_SUBTRACT: u32 = 7u;
const BLEND_OVERLAY: u32 = 8u;
const BLEND_EXCLUSION: u32 = 9u;

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
        case BLEND_LIGHTEN: {
            return max(cb, cs);
        }
        case BLEND_DARKEN: {
            return min(cb, cs);
        }
        case BLEND_DIFFERENCE: {
            return abs(cb - cs);
        }
        case BLEND_SUBTRACT: {
            return max(cb - cs, vec3<f32>(0.0));
        }
        case BLEND_OVERLAY: {
            // Hard light with the roles swapped: driven by the layers below.
            let dark = 2.0 * cb * cs;
            let light = 1.0 - 2.0 * (1.0 - cb) * (1.0 - cs);
            return select(light, dark, cb <= vec3<f32>(0.5));
        }
        case BLEND_EXCLUSION: {
            return cb + cs - 2.0 * cb * cs;
        }
        default: {
            return cs;
        }
    }
}

// Composites one layer, premultiplied, over the colors below it.
fn composite(below: vec4<f32>, layer: vec4<f32>, params: Layer) -> vec4<f32> {
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var result = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if batch.over_black == 0u {
        result = textureLoad(below_texture, vec2<i32>(in.position.xy), 0);
    }
    // The current frame of each layer is mixed with the clip it replaces,
    // by the progress of the transition, before blending.
    if batch.count > 0u {
        let layer = mix(
            textureSample(previous0, layer_sampler, in.uv),
            textureSample(layer0, layer_sampler, in.uv),
            batch.layers[0].progress,
        );
        result = composite(result, layer, batch.layers[0]);
    }
    if batch.count > 1u {
        let layer = mix(
            textureSample(previous1, layer_sampler, in.uv),
            textureSample(layer1, layer_sampler, in.uv),
            batch.layers[1].progress,
        );
        result = composite(result, layer, batch.layers[1]);
    }
    if batch.count > 2u {
        let layer = mix(
            textureSample(previous2, layer_sampler, in.uv),
            textureSample(layer2, layer_sampler, in.uv),
            batch.layers[2].progress,
        );
        result = composite(result, layer, batch.layers[2]);
    }
    if batch.count > 3u {
        let layer = mix(
            textureSample(previous3, layer_sampler, in.uv),
            textureSample(layer3, layer_sampler, in.uv),
            batch.layers[3].progress,
        );
        result = composite(result, layer, batch.layers[3]);
    }
    return result;
}
