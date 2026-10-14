// One half of the image reflected onto the other.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;
    switch u32(param(0u)) {
        // Left half onto the right.
        case 0u: {
            uv.x = min(uv.x, 1.0 - uv.x);
        }
        // Right half onto the left.
        case 1u: {
            uv.x = max(uv.x, 1.0 - uv.x);
        }
        // Top half onto the bottom.
        case 2u: {
            uv.y = min(uv.y, 1.0 - uv.y);
        }
        // Bottom half onto the top.
        default: {
            uv.y = max(uv.y, 1.0 - uv.y);
        }
    }
    return sample_clamped(uv);
}
