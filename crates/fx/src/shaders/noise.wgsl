// Smooth noise: random values on a grid, blended between cells and
// between moments that change with the beat.

fn cell_value(cell: vec2<f32>, moment: f32) -> f32 {
    return hash(cell.x * 157.0 + cell.y * 113.0 + moment * 71.0);
}

fn smooth_noise(point: vec2<f32>, moment: f32) -> f32 {
    let cell = floor(point);
    let t = point - cell;
    let blend = t * t * (vec2<f32>(3.0) - 2.0 * t);
    let a = cell_value(cell, moment);
    let b = cell_value(cell + vec2<f32>(1.0, 0.0), moment);
    let c = cell_value(cell + vec2<f32>(0.0, 1.0), moment);
    let d = cell_value(cell + vec2<f32>(1.0, 1.0), moment);
    return mix(mix(a, b, blend.x), mix(c, d, blend.x), blend.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = params.size.x / max(params.size.y, 1.0);
    let point = in.uv * vec2<f32>(param(0u) * aspect, param(0u));
    let time = params.beat * param(1u);
    let moment = floor(time);
    let t = time - moment;
    let blend = t * t * (3.0 - 2.0 * t);
    if u32(param(2u)) == 0u {
        let level = mix(smooth_noise(point, moment), smooth_noise(point, moment + 1.0), blend);
        return vec4<f32>(vec3<f32>(level), 1.0);
    }
    var rgb = vec3<f32>(0.0);
    for (var channel = 0u; channel < 3u; channel += 1u) {
        let shift = f32(channel) * 37.0;
        rgb[channel] = mix(
            smooth_noise(point + vec2<f32>(shift), moment),
            smooth_noise(point + vec2<f32>(shift), moment + 1.0),
            blend,
        );
    }
    return vec4<f32>(rgb, 1.0);
}
