// The program image (16-bit float, premultiplied) copied pixel for pixel
// into an 8-bit BGRA image for other applications.

@group(0) @binding(0) var program: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return clamp(textureLoad(program, vec2<i32>(position.xy), 0), vec4<f32>(0.0), vec4<f32>(1.0));
}
