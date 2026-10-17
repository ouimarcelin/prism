// Draws slices of the program image on a screen. Each vertex carries its
// position on the screen (fractions from the top left) and its position in
// the program multiplied by a perspective weight q, with q last: dividing
// by the interpolated q gives a perspective-correct image in any quad.
//
// The instance index names the slice's mask: a polygon in pixels of the
// screen, inside which the slice shows (outside when inverted), with a
// soft edge. Mask 0 is none.

@group(0) @binding(0) var program: texture_2d<f32>;
@group(0) @binding(1) var program_sampler: sampler;

struct Mask {
    first: u32,
    count: u32,
    inverted: u32,
    feather: f32,
}

@group(0) @binding(2) var<storage, read> masks: array<Mask>;
@group(0) @binding(3) var<storage, read> mask_points: array<vec2<f32>>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) source: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) source: vec3<f32>,
    @location(1) @interpolate(flat) mask: u32,
}

@vertex
fn vs_main(in: VertexInput, @builtin(instance_index) mask: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.position.x * 2.0 - 1.0, 1.0 - in.position.y * 2.0, 0.0, 1.0);
    out.source = in.source;
    out.mask = mask;
    return out;
}

// How much of the pixel at `pixel` the mask lets through, from 0 to 1.
fn coverage(index: u32, pixel: vec2<f32>) -> f32 {
    let mask = masks[index];
    if mask.count < 3u {
        return 1.0;
    }
    var inside = false;
    var distance = 1e9;
    for (var i = 0u; i < mask.count; i += 1u) {
        let a = mask_points[mask.first + i];
        let b = mask_points[mask.first + (i + 1u) % mask.count];
        if (a.y > pixel.y) != (b.y > pixel.y) {
            let x = a.x + (pixel.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if pixel.x < x {
                inside = !inside;
            }
        }
        let edge = b - a;
        let t = clamp(dot(pixel - a, edge) / max(dot(edge, edge), 1e-6), 0.0, 1.0);
        distance = min(distance, length(pixel - (a + edge * t)));
    }
    var signed = select(-distance, distance, inside);
    if mask.inverted != 0u {
        signed = -signed;
    }
    // At least a pixel of soft edge, so the outline is not jagged.
    let feather = max(mask.feather, 1.0);
    return clamp(signed / feather + 0.5, 0.0, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.source.xy / in.source.z;
    let color = textureSample(program, program_sampler, uv);
    return color * coverage(in.mask, in.position.xy);
}
