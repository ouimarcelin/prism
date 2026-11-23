// 3D objects of a screen: a mesh seen from the screen's projector, the
// program laid on it by its texture coordinates.

struct Object {
    // Model, view and projection, column-major.
    transform: mat4x4<f32>,
    // Area of the program on the texture coordinates: left, top, width,
    // height.
    area: vec4<f32>,
    opacity: f32,
}

@group(0) @binding(0) var program: texture_2d<f32>;
@group(0) @binding(1) var program_sampler: sampler;
@group(0) @binding(2) var<uniform> object: Object;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = object.transform * vec4<f32>(position, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = object.area.xy + fract(in.uv) * object.area.zw;
    // Premultiplied: the opacity scales every channel.
    return textureSample(program, program_sampler, uv) * object.opacity;
}
