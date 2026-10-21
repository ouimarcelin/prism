// Shared by the generators of the procedural bank. Each generator declares
// `fn field(p: vec2<f32>, t: f32) -> f32`, a value from 0 to 1 at point `p`
// of its plane and time `t`; this file places the plane (zoom, position,
// rotation, stretch, seed), runs the time (speed, beat or seconds) and
// turns the value into colours (palette, hue, saturation, contrast,
// brightness, inversion, levels, transparent black). `COMMON`, the index
// of the first shared setting, is declared before this file.
//
// Noises: integer hash (pcg3d), value noise, Perlin and simplex noise in
// three dimensions (time is the third), fractal sums, Worley cells.
// Waves: one cycle of a sine, triangle, square or sawtooth.

const C_ZOOM: u32 = 0u;
const C_OFFSET_X: u32 = 1u;
const C_OFFSET_Y: u32 = 2u;
const C_ANGLE: u32 = 3u;
const C_STRETCH: u32 = 4u;
const C_SPEED: u32 = 5u;
const C_TIME_UNIT: u32 = 6u;
const C_SEED: u32 = 7u;
const C_PALETTE: u32 = 8u;
const C_HUE: u32 = 9u;
const C_SATURATION: u32 = 10u;
const C_CONTRAST: u32 = 11u;
const C_BRIGHTNESS: u32 = 12u;
const C_INVERT: u32 = 13u;
const C_LEVELS: u32 = 14u;
const C_ALPHA: u32 = 15u;

fn bank_setting(index: u32) -> f32 {
    return param(COMMON + index);
}

// ---- Hashing ----

// Three well mixed 32-bit values from three integers (pcg3d, Jarzynski and
// Olano, "Hash Functions for GPU Rendering", 2020).
fn pcg3d(input: vec3<u32>) -> vec3<u32> {
    var v = input * 1664525u + vec3<u32>(1013904223u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return v;
}

// Three values in 0..1 for a point of the integer lattice.
fn hash33(cell: vec3<f32>) -> vec3<f32> {
    let bits = vec3<u32>(vec3<i32>(cell));
    return vec3<f32>(pcg3d(bits)) * (1.0 / 4294967295.0);
}

fn hash31(cell: vec3<f32>) -> f32 {
    return hash33(cell).x;
}

// A random direction for a lattice point, for gradient noises.
fn gradient(cell: vec3<f32>) -> vec3<f32> {
    return normalize(hash33(cell) * 2.0 - vec3<f32>(1.0) + vec3<f32>(1e-4));
}

// Quintic fade: no seam in the first and second derivatives.
fn fade(t: vec3<f32>) -> vec3<f32> {
    return t * t * t * (t * (t * 6.0 - vec3<f32>(15.0)) + vec3<f32>(10.0));
}

// ---- Noises, from -1 to 1 ----

fn value_noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fade(p - cell);
    let v000 = hash31(cell);
    let v100 = hash31(cell + vec3<f32>(1.0, 0.0, 0.0));
    let v010 = hash31(cell + vec3<f32>(0.0, 1.0, 0.0));
    let v110 = hash31(cell + vec3<f32>(1.0, 1.0, 0.0));
    let v001 = hash31(cell + vec3<f32>(0.0, 0.0, 1.0));
    let v101 = hash31(cell + vec3<f32>(1.0, 0.0, 1.0));
    let v011 = hash31(cell + vec3<f32>(0.0, 1.0, 1.0));
    let v111 = hash31(cell + vec3<f32>(1.0, 1.0, 1.0));
    let near = mix(mix(v000, v100, f.x), mix(v010, v110, f.x), f.y);
    let far = mix(mix(v001, v101, f.x), mix(v011, v111, f.x), f.y);
    return mix(near, far, f.z) * 2.0 - 1.0;
}

// Gradient noise (Perlin, "Improving Noise", 2002), random gradients.
fn perlin_noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let x = p - cell;
    let f = fade(x);
    var corners: array<f32, 8>;
    for (var index = 0u; index < 8u; index += 1u) {
        let offset = vec3<f32>(f32(index & 1u), f32((index >> 1u) & 1u), f32(index >> 2u));
        corners[index] = dot(gradient(cell + offset), x - offset);
    }
    let near = mix(mix(corners[0], corners[1], f.x), mix(corners[2], corners[3], f.x), f.y);
    let far = mix(mix(corners[4], corners[5], f.x), mix(corners[6], corners[7], f.x), f.y);
    return clamp(mix(near, far, f.z) * 1.15, -1.0, 1.0);
}

// Simplex noise in three dimensions (Perlin 2001; Gustavson, "Simplex
// noise demystified", 2005): four corners of a tetrahedron per point.
fn simplex_noise(p: vec3<f32>) -> f32 {
    let skew = 1.0 / 3.0;
    let unskew = 1.0 / 6.0;
    let cell = floor(p + vec3<f32>(dot(p, vec3<f32>(skew))));
    let x0 = p - cell + vec3<f32>(dot(cell, vec3<f32>(unskew)));
    // Which tetrahedron of the skewed cube: corners by rank of the axes.
    var i1: vec3<f32>;
    var i2: vec3<f32>;
    if x0.x >= x0.y {
        if x0.y >= x0.z {
            i1 = vec3<f32>(1.0, 0.0, 0.0);
            i2 = vec3<f32>(1.0, 1.0, 0.0);
        } else if x0.x >= x0.z {
            i1 = vec3<f32>(1.0, 0.0, 0.0);
            i2 = vec3<f32>(1.0, 0.0, 1.0);
        } else {
            i1 = vec3<f32>(0.0, 0.0, 1.0);
            i2 = vec3<f32>(1.0, 0.0, 1.0);
        }
    } else {
        if x0.y < x0.z {
            i1 = vec3<f32>(0.0, 0.0, 1.0);
            i2 = vec3<f32>(0.0, 1.0, 1.0);
        } else if x0.x < x0.z {
            i1 = vec3<f32>(0.0, 1.0, 0.0);
            i2 = vec3<f32>(0.0, 1.0, 1.0);
        } else {
            i1 = vec3<f32>(0.0, 1.0, 0.0);
            i2 = vec3<f32>(1.0, 1.0, 0.0);
        }
    }
    let x1 = x0 - i1 + vec3<f32>(unskew);
    let x2 = x0 - i2 + vec3<f32>(2.0 * unskew);
    let x3 = x0 - vec3<f32>(1.0 - 3.0 * unskew);
    var sum = 0.0;
    let offsets = array<vec3<f32>, 4>(vec3<f32>(0.0), i1, i2, vec3<f32>(1.0));
    let points = array<vec3<f32>, 4>(x0, x1, x2, x3);
    for (var corner = 0u; corner < 4u; corner += 1u) {
        let x = points[corner];
        let t = max(0.6 - dot(x, x), 0.0);
        let t2 = t * t;
        sum += t2 * t2 * dot(gradient(cell + offsets[corner]), x);
    }
    return clamp(sum * 32.0, -1.0, 1.0);
}

// Base noise: 0 value, 1 Perlin, 2 simplex.
fn base_noise(p: vec3<f32>, base: u32) -> f32 {
    switch base {
        case 0u: {
            return value_noise(p);
        }
        case 1u: {
            return perlin_noise(p);
        }
        default: {
            return simplex_noise(p);
        }
    }
}

// Fractal sum of `octaves` noises, each `lacunarity` times finer and `gain`
// times weaker, from 0 to 1. Modes: 0 fBm, 1 turbulence (folded),
// 2 ridges (sharpened by `sharpness`, each octave weighted by the one
// before), 3 billow (puffy).
fn fractal(p: vec3<f32>, base: u32, mode: u32, octaves: f32, lacunarity: f32, gain: f32, sharpness: f32) -> f32 {
    var sum = 0.0;
    var norm = 0.0;
    var amplitude = 1.0;
    var frequency = 1.0;
    var weight = 1.0;
    let count = u32(clamp(round(octaves), 1.0, 10.0));
    for (var octave = 0u; octave < count; octave += 1u) {
        // Each octave shifted, so their lattices do not line up.
        let shift = vec3<f32>(f32(octave) * 17.31, f32(octave) * -9.73, f32(octave) * 5.17);
        let n = base_noise(p * frequency + shift, base);
        var signal: f32;
        switch mode {
            case 1u: {
                signal = abs(n);
            }
            case 2u: {
                signal = pow(1.0 - abs(n), sharpness) * weight;
                weight = clamp(signal * 2.0, 0.0, 1.0);
            }
            case 3u: {
                signal = sqrt(abs(n));
            }
            default: {
                signal = 0.5 + 0.5 * n;
            }
        }
        sum += signal * amplitude;
        norm += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    return clamp(sum / max(norm, 1e-6), 0.0, 1.0);
}

// Worley cells of a plane: distances to the nearest and second nearest
// feature point (one per unit cell, moved by `jitter` and swinging with
// time by `motion`), and a value from 0 to 1 for the nearest one's cell.
// Metrics: 0 Euclidean, 1 Manhattan, 2 Chebyshev.
fn worley(p: vec2<f32>, t: f32, jitter: f32, motion: f32, metric: u32) -> vec3<f32> {
    let cell = floor(p);
    var f1 = 1e9;
    var f2 = 1e9;
    var id = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let neighbour = cell + vec2<f32>(f32(x), f32(y));
            let random = hash33(vec3<f32>(neighbour, 0.0));
            let swing = 0.5 * sin(t * motion + random.z * 2.0 * PI) * jitter;
            let point = neighbour + vec2<f32>(0.5) + (random.xy - vec2<f32>(0.5)) * jitter
                + vec2<f32>(swing, -swing) * 0.5;
            let d = point - p;
            var distance: f32;
            switch metric {
                case 1u: {
                    distance = abs(d.x) + abs(d.y);
                }
                case 2u: {
                    distance = max(abs(d.x), abs(d.y));
                }
                default: {
                    distance = length(d);
                }
            }
            if distance < f1 {
                f2 = f1;
                f1 = distance;
                id = random.z;
            } else if distance < f2 {
                f2 = distance;
            }
        }
    }
    return vec3<f32>(f1, f2, id);
}

// ---- Waves ----

// One cycle of a wave at `phase` (in cycles), from 0 to 1. Shapes: 0 sine,
// 1 triangle (rising for `duty` of the cycle), 2 square (on for `duty` of
// the cycle, edges blurred by `soft`), 3 sawtooth.
fn wave_shape(phase: f32, shape: u32, duty: f32, soft: f32) -> f32 {
    let x = fract(phase);
    let d = clamp(duty, 0.01, 0.99);
    switch shape {
        case 1u: {
            return select((1.0 - x) / (1.0 - d), x / d, x < d);
        }
        case 2u: {
            let edge = max(soft, 1e-3) * 0.5 * min(d, 1.0 - d);
            let from_centre = abs(fract(phase - d * 0.5 + 0.5) - 0.5);
            return 1.0 - smoothstep(d * 0.5 - edge, d * 0.5 + edge, from_centre);
        }
        case 3u: {
            return x;
        }
        default: {
            return 0.5 - 0.5 * cos(2.0 * PI * x);
        }
    }
}

// ---- Space, time, colour ----

// The point of the generator's plane under `uv`: centred, square pixels,
// then the shared settings of space.
fn bank_point(uv: vec2<f32>) -> vec2<f32> {
    let aspect = params.size.x / max(params.size.y, 1.0);
    var q = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    q = rotate(q, -radians(bank_setting(C_ANGLE)));
    let stretch = exp2(bank_setting(C_STRETCH));
    q = q * vec2<f32>(1.0 / stretch, stretch);
    q = q / max(bank_setting(C_ZOOM), 1e-3);
    let seed = bank_setting(C_SEED);
    return q + vec2<f32>(bank_setting(C_OFFSET_X), bank_setting(C_OFFSET_Y)) + vec2<f32>(seed * 7.31, seed * -3.17);
}

// Time of the generator: beats of the tempo, or seconds, times the speed.
fn bank_time() -> f32 {
    let clock = select(params.beat, params.time, bank_setting(C_TIME_UNIT) > 0.5);
    return clock * bank_setting(C_SPEED);
}

// Cosine palette a + b cos(2π (c x + d)).
fn cosine(x: f32, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return a + b * cos(2.0 * PI * (c * x + d));
}

// Colour of value `x` (0 to 1) in palette `index`.
fn palette(x: f32, index: u32) -> vec3<f32> {
    let half = vec3<f32>(0.5);
    // Cosine palettes come back to their first colour after a whole cycle:
    // read them over 80 % of it, so that 0 and 1 stay apart.
    let y = x * 0.8;
    switch index {
        // Rainbow.
        case 1u: {
            return cosine(y, half, half, vec3<f32>(1.0), vec3<f32>(0.0, 0.33, 0.67));
        }
        // Fire: black, red, yellow, white.
        case 2u: {
            return clamp(vec3<f32>(3.0 * x, 3.0 * x - 1.0, 3.0 * x - 2.0), vec3<f32>(0.0), vec3<f32>(1.0));
        }
        // Ice: black, blue, cyan, white.
        case 3u: {
            return clamp(vec3<f32>(3.0 * x - 2.0, 3.0 * x - 1.0, 3.0 * x), vec3<f32>(0.0), vec3<f32>(1.0));
        }
        // Ocean: deep blue to foam.
        case 4u: {
            return mix(vec3<f32>(0.0, 0.05, 0.2), vec3<f32>(0.0, 0.6, 0.75), smoothstep(0.0, 0.7, x))
                + vec3<f32>(smoothstep(0.75, 1.0, x));
        }
        // Neon.
        case 5u: {
            return cosine(y, half, half, vec3<f32>(2.0, 1.0, 0.0), vec3<f32>(0.5, 0.2, 0.25));
        }
        // Sunset.
        case 6u: {
            return cosine(y, vec3<f32>(0.8, 0.5, 0.4), vec3<f32>(0.2, 0.4, 0.2), vec3<f32>(2.0, 1.0, 1.0), vec3<f32>(0.0, 0.25, 0.25));
        }
        default: {
            return vec3<f32>(x);
        }
    }
}

// Turns `color` around the grey axis by `angle` radians.
fn hue_rotate(color: vec3<f32>, angle: f32) -> vec3<f32> {
    let axis = vec3<f32>(0.57735027);
    let c = cos(angle);
    return color * c + cross(axis, color) * sin(angle) + axis * dot(axis, color) * (1.0 - c);
}

// The colour of field value `v`, premultiplied.
fn bank_color(v: f32) -> vec4<f32> {
    var x = clamp(v, 0.0, 1.0);
    x = clamp((x - 0.5) * bank_setting(C_CONTRAST) + 0.5, 0.0, 1.0);
    if bank_setting(C_INVERT) > 0.5 {
        x = 1.0 - x;
    }
    let levels = round(bank_setting(C_LEVELS));
    if levels >= 2.0 {
        x = min(floor(x * levels), levels - 1.0) / (levels - 1.0);
    }
    var rgb = palette(x, u32(bank_setting(C_PALETTE)));
    rgb = hue_rotate(rgb, radians(bank_setting(C_HUE)));
    rgb = mix(vec3<f32>(dot(rgb, LUMA)), rgb, bank_setting(C_SATURATION));
    rgb = clamp(rgb * bank_setting(C_BRIGHTNESS), vec3<f32>(0.0), vec3<f32>(1.0));
    // Transparent black: alpha from the brightest channel, so the colours
    // stay premultiplied.
    let alpha = mix(1.0, max(rgb.r, max(rgb.g, rgb.b)), bank_setting(C_ALPHA));
    return vec4<f32>(rgb, alpha);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return bank_color(field(bank_point(in.uv), bank_time()));
}
