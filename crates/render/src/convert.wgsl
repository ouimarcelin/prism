// Converts a decoded video frame into a layer texture: premultiplied RGBA
// in display space, exactly the frame size (block padding is dropped).

struct ConvertParams {
    color_layout: u32,
    has_alpha_plane: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<uniform> params: ConvertParams;
@group(0) @binding(1) var color_texture: texture_2d<f32>;
@group(0) @binding(2) var alpha_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

const LAYOUT_RGB: u32 = 0u;
const LAYOUT_RGBA: u32 = 1u;
const LAYOUT_SCALED_YCOCG: u32 = 2u;

// Scaled YCoCg as defined for HAP Q: Co in red, Cg in green, scale in blue,
// Y in alpha, chroma offset by 128/255.
fn scaled_ycocg_to_rgb(texel: vec4<f32>) -> vec3<f32> {
    let scale = texel.b * (255.0 / 8.0) + 1.0;
    let co = (texel.r - 128.0 / 255.0) / scale;
    let cg = (texel.g - 128.0 / 255.0) / scale;
    let y = texel.a;
    return vec3<f32>(y + co - cg, y + cg, y - co - cg);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(position.xy);
    let color = textureLoad(color_texture, texel, 0);

    var rgb = color.rgb;
    var alpha = 1.0;
    switch params.color_layout {
        case LAYOUT_RGBA: {
            alpha = color.a;
        }
        case LAYOUT_SCALED_YCOCG: {
            rgb = clamp(scaled_ycocg_to_rgb(color), vec3<f32>(0.0), vec3<f32>(1.0));
        }
        default: {}
    }
    if params.has_alpha_plane != 0u {
        alpha = textureLoad(alpha_texture, texel, 0).r;
    }
    return vec4<f32>(rgb * alpha, alpha);
}
