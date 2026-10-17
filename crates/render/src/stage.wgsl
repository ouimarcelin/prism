// Draws slices of the program image on a screen. Each vertex carries its
// position on the screen (fractions from the top left) and its position in
// the program multiplied by a perspective weight q, with q last: dividing
// by the interpolated q gives a perspective-correct image in any quad.

@group(0) @binding(0) var program: texture_2d<f32>;
@group(0) @binding(1) var program_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) source: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) source: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.position.x * 2.0 - 1.0, 1.0 - in.position.y * 2.0, 0.0, 1.0);
    out.source = in.source;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.source.xy / in.source.z;
    return textureSample(program, program_sampler, uv);
}
