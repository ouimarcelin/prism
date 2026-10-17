// Draws slices of the program image on a screen. Each vertex carries its
// position on the screen (fractions from the top left) and its position in
// the program multiplied by a perspective weight q, with q last: dividing
// by the interpolated q gives a perspective-correct image in any quad.
//
// The instance index names the slice: its mask (a polygon in pixels of the
// screen, inside which the slice shows, outside when inverted, with a soft
// edge), its edge fades for overlapping projectors, and whether it shows a
// test pattern instead of the program.

@group(0) @binding(0) var program: texture_2d<f32>;
@group(0) @binding(1) var program_sampler: sampler;

struct Slice {
    first: u32,
    count: u32,
    inverted: u32,
    feather: f32,
    // Area of the composition: x, y, width, height.
    input: vec4<f32>,
    // Fade widths: left, right, top, bottom, as fractions of the slice.
    edges: vec4<f32>,
    // Fade power, gamma, test pattern (1 or 0), slice index.
    curve: vec4<f32>,
}

@group(0) @binding(2) var<storage, read> slices: array<Slice>;
@group(0) @binding(3) var<storage, read> mask_points: array<vec2<f32>>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) source: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) source: vec3<f32>,
    @location(1) @interpolate(flat) slice: u32,
}

@vertex
fn vs_main(in: VertexInput, @builtin(instance_index) slice: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.position.x * 2.0 - 1.0, 1.0 - in.position.y * 2.0, 0.0, 1.0);
    out.source = in.source;
    out.slice = slice;
    return out;
}

// How much of the pixel at `pixel` the mask lets through, from 0 to 1.
fn coverage(slice: Slice, pixel: vec2<f32>) -> f32 {
    if slice.count < 3u {
        return 1.0;
    }
    var inside = false;
    var distance = 1e9;
    for (var i = 0u; i < slice.count; i += 1u) {
        let a = mask_points[slice.first + i];
        let b = mask_points[slice.first + (i + 1u) % slice.count];
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
    if slice.inverted != 0u {
        signed = -signed;
    }
    // At least a pixel of soft edge, so the outline is not jagged.
    let feather = max(slice.feather, 1.0);
    return clamp(signed / feather + 0.5, 0.0, 1.0);
}

// Light kept across a fade, `t` from 0 at the outer edge to 1 at the inner
// one, as a pixel value (see EdgeBlend::factor).
fn fade(t: f32, power: f32, gamma: f32) -> f32 {
    let x = clamp(t, 0.0, 1.0);
    var light: f32;
    if x < 0.5 {
        light = 0.5 * pow(2.0 * x, power);
    } else {
        light = 1.0 - 0.5 * pow(2.0 * (1.0 - x), power);
    }
    return pow(light, 1.0 / gamma);
}

fn edge_fade(slice: Slice, local: vec2<f32>) -> f32 {
    var kept = 1.0;
    let power = slice.curve.x;
    let gamma = slice.curve.y;
    if slice.edges.x > 0.0 {
        kept *= fade(local.x / slice.edges.x, power, gamma);
    }
    if slice.edges.y > 0.0 {
        kept *= fade((1.0 - local.x) / slice.edges.y, power, gamma);
    }
    if slice.edges.z > 0.0 {
        kept *= fade(local.y / slice.edges.z, power, gamma);
    }
    if slice.edges.w > 0.0 {
        kept *= fade((1.0 - local.y) / slice.edges.w, power, gamma);
    }
    return kept;
}

// A grid over the slice, to line projectors up: eight cells each way, a
// cross in the middle, a border, tinted by slice.
fn pattern(local: vec2<f32>, index: f32) -> vec4<f32> {
    let cells = local * 8.0;
    let width = max(fwidth(cells), vec2<f32>(1e-4));
    let grid = abs(fract(cells + 0.5) - 0.5) / width;
    let line = 1.0 - clamp(min(grid.x, grid.y) - 0.5, 0.0, 1.0);
    let middle = abs(local - 0.5) / max(fwidth(local), vec2<f32>(1e-6));
    let cross = 1.0 - clamp(min(middle.x, middle.y) - 1.5, 0.0, 1.0);
    let border_distance = min(min(local.x, 1.0 - local.x), min(local.y, 1.0 - local.y));
    let border = 1.0 - clamp(border_distance / max(fwidth(local.x), 1e-6) - 2.0, 0.0, 1.0);
    let hue = fract(index * 0.618) * 6.0;
    let tint = clamp(
        vec3<f32>(abs(hue - 3.0) - 1.0, 2.0 - abs(hue - 2.0), 2.0 - abs(hue - 4.0)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    var color = vec3<f32>(0.12) + tint * 0.25;
    color = mix(color, vec3<f32>(0.85), line);
    color = mix(color, tint, cross);
    color = mix(color, vec3<f32>(1.0), border);
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let slice = slices[in.slice];
    let uv = in.source.xy / in.source.z;
    let local = (uv - slice.input.xy) / max(slice.input.zw, vec2<f32>(1e-6));
    let image = textureSample(program, program_sampler, uv);
    let test = pattern(local, slice.curve.w);
    let color = select(image, test, slice.curve.z > 0.5);
    return color * edge_fade(slice, local) * coverage(slice, in.position.xy);
}
