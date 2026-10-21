// Standing waves (Chladni figures): the nodal lines of a vibrating plate,
// square or round, for modes n and m; the modes sway with time.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let wobble = param(4u) * 0.5;
    let n = param(0u) + wobble * sin(t * 0.5 * PI);
    let m = param(1u) + wobble * cos(t * 0.5 * PI);
    let q = p * 2.0;
    var value: f32;
    if u32(param(2u)) == 1u {
        let r = length(q);
        let a = atan2(q.y, q.x);
        value = cos(round(n) * a) * sin(m * PI * r);
    } else {
        value = cos(n * PI * q.x) * cos(m * PI * q.y) - cos(m * PI * q.x) * cos(n * PI * q.y);
    }
    let width = mix(0.6, 0.02, param(3u));
    return 1.0 - smoothstep(0.0, width, abs(value));
}
