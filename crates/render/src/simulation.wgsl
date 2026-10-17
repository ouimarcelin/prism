// Rehearsal of overlapping projectors: each screen image is added, in
// light (its values raised to the gamma), into an accumulation image at its
// place; the sum is then turned back into pixel values.

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
// x: gamma of the simulated projectors.
@group(0) @binding(2) var<uniform> params: vec4<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle covering the viewport.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: VertexOutput;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

@fragment
fn fs_accumulate(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(image, image_sampler, in.uv);
    return vec4<f32>(pow(max(color.rgb, vec3<f32>(0.0)), vec3<f32>(params.x)), 1.0);
}

@fragment
fn fs_resolve(in: VertexOutput) -> @location(0) vec4<f32> {
    let light = textureSample(image, image_sampler, in.uv);
    return vec4<f32>(pow(max(light.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / params.x)), 1.0);
}
