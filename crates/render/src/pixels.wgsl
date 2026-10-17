// Colours of the program image at given points, one per texel of a small
// image read back by the CPU (pixel mapping). Texel i, row by row, takes
// point i; texels past the last point are black.

@group(0) @binding(0) var program: texture_2d<f32>;
@group(0) @binding(1) var program_sampler: sampler;
@group(0) @binding(2) var<storage, read> points: array<vec2<f32>>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let width = u32(256);
    let index = u32(position.y) * width + u32(position.x);
    let count = arrayLength(&points);
    let point = points[min(index, count - 1u)];
    let color = textureSampleLevel(program, program_sampler, point, 0.0);
    let inside = select(0.0, 1.0, index < count);
    return vec4<f32>(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * inside, 1.0);
}
