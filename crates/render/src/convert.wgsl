// Converts a decoded video frame into a layer texture: premultiplied RGBA
// in display space, exactly the frame size (block padding is dropped).

struct ConvertParams {
    color_layout: u32,
    has_alpha_plane: u32,
    // Bit 0: full range; bit 1: BT.601 (otherwise BT.709).
    yuv_flags: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: ConvertParams;
// Color plane; for 4:2:0 video, the luma plane.
@group(0) @binding(1) var color_texture: texture_2d<f32>;
@group(0) @binding(2) var alpha_texture: texture_2d<f32>;
// Interleaved Cb, Cr at half resolution, for 4:2:0 video.
@group(0) @binding(3) var chroma_texture: texture_2d<f32>;
@group(0) @binding(4) var chroma_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

const LAYOUT_RGB: u32 = 0u;
const LAYOUT_RGBA: u32 = 1u;
const LAYOUT_SCALED_YCOCG: u32 = 2u;
const LAYOUT_YUV_420: u32 = 3u;

// Luma and chroma to color; must match `YuvImage::rgb_at`.
fn yuv_to_rgb(luma: f32, chroma: vec2<f32>, flags: u32) -> vec3<f32> {
    var y = luma;
    var c = chroma - vec2<f32>(128.0 / 255.0);
    if (flags & 1u) == 0u {
        y = (luma - 16.0 / 255.0) * (255.0 / 219.0);
        c = c * (255.0 / 224.0);
    }
    var rgb: vec3<f32>;
    if (flags & 2u) != 0u {
        rgb = vec3<f32>(y + 1.402 * c.y, y - 0.344136 * c.x - 0.714136 * c.y, y + 1.772 * c.x);
    } else {
        rgb = vec3<f32>(y + 1.5748 * c.y, y - 0.187324 * c.x - 0.468124 * c.y, y + 1.8556 * c.x);
    }
    return clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
}

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
        case LAYOUT_YUV_420: {
            let size = vec2<f32>(textureDimensions(color_texture));
            let chroma = textureSampleLevel(chroma_texture, chroma_sampler, position.xy / size, 0.0).rg;
            rgb = yuv_to_rgb(color.r, chroma, params.yuv_flags);
        }
        default: {}
    }
    if params.has_alpha_plane != 0u {
        alpha = textureLoad(alpha_texture, texel, 0).r;
    }
    return vec4<f32>(rgb * alpha, alpha);
}
