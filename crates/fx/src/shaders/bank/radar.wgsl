// Radar sweep: a turning beam with an afterglow, range rings and targets
// lit as the beam passes them.

fn radar_behind(angle: f32, t: f32) -> f32 {
    // How far behind the beam, in turns (0 on the beam).
    return fract(t * 0.25 - angle / (2.0 * PI));
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let r = length(p);
    let afterglow = max(param(0u), 1e-3);
    let beam = exp(-radar_behind(atan2(p.y, p.x), t) / afterglow * 3.0);
    var value = beam * 0.8;
    let rings = round(param(1u));
    if rings > 0.0 {
        let ring = abs(fract(r / 0.48 * rings + 0.5) - 0.5) / rings * 0.48;
        value += 0.35 * (1.0 - smoothstep(0.0, bank_pixel() * 2.0, ring));
    }
    // Targets: one possible per cell of a grid.
    let cells = 10.0;
    let cell = floor(p * cells);
    let random = hash33(vec3<f32>(cell, 7.0));
    if random.z < param(2u) {
        let blip = (cell + vec2<f32>(0.2) + random.xy * 0.6) / cells;
        let lit = exp(-radar_behind(atan2(blip.y, blip.x), t) / afterglow * 2.0);
        value += lit * (1.0 - smoothstep(0.006, 0.012, length(p - blip)));
    }
    let screen = 1.0 - smoothstep(0.48, 0.48 + bank_pixel() * 2.0, r);
    return clamp(value, 0.0, 1.0) * screen;
}
