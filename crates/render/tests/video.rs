//! HAP frames uploaded to the GPU and converted into layer textures.
//!
//! Needs a GPU with BC texture support; otherwise the tests print a notice
//! and pass.

#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use prism_core::{BlendMode, Resolution};
use prism_media::test_support::{
    Compression, Image, MovOptions, encode_image, encode_multiple, plane_from_fn, solid_plane,
    write_mov, ycocg_color,
};
use prism_media::{BlockFormat, Frame, HapDecoder, HapFrame, RgbaImage};
use prism_render::{Compositor, FrameUploader, Gpu, LayerInput, VideoTexture, read_texture};

/// BC endpoints are 5 or 6 bits per channel; GPUs expand them slightly
/// differently.
const TOLERANCE: f32 = 0.02;

fn gpu() -> Option<Gpu> {
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            return None;
        }
    };
    if !gpu.supports_block_compression() {
        eprintln!("skipping GPU test: no BC texture support");
        return None;
    }
    Some(gpu)
}

/// Value of an RGB565 channel of `bits` bits, as a GPU decodes it.
fn quantized(value: u8, bits: u32) -> f32 {
    let levels = (1u32 << bits) - 1;
    let q = u32::from(value) >> (8 - bits);
    q as f32 / levels as f32
}

fn quantized_rgb(rgb: [u8; 3]) -> [f32; 3] {
    [
        quantized(rgb[0], 5),
        quantized(rgb[1], 6),
        quantized(rgb[2], 5),
    ]
}

/// Converts `frame` and returns its pixels, row by row.
fn convert(gpu: &Gpu, frame: &Frame, width: u32, height: u32) -> Vec<[f32; 4]> {
    let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
    let mut target = VideoTexture::new(&gpu.device, width, height);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    uploader
        .upload(&gpu.device, &gpu.queue, &mut encoder, &mut target, frame)
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    read_texture(&gpu.device, &gpu.queue, target.texture()).unwrap()
}

fn decode(data: &[u8], width: u32, height: u32) -> Frame {
    let mut frame = HapFrame::default();
    prism_media::hap::decode_frame(data, width, height, &mut frame).unwrap();
    Frame::Hap(frame)
}

fn assert_pixel(pixels: &[[f32; 4]], width: u32, x: u32, y: u32, expected: [f32; 4]) {
    let pixel = pixels[(y * width + x) as usize];
    for channel in 0..4 {
        assert!(
            (pixel[channel] - expected[channel]).abs() < TOLERANCE,
            "pixel ({x}, {y}): expected {expected:?}, got {pixel:?}"
        );
    }
}

const QUADRANTS: [[u8; 4]; 4] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 255, 255],
];

fn opaque(rgb: [f32; 3]) -> [f32; 4] {
    [rgb[0], rgb[1], rgb[2], 1.0]
}

#[test]
fn hap_frame_keeps_its_orientation() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (32, 16);
    let texture = plane_from_fn(BlockFormat::Bc1, width, height, |bx, by| {
        QUADRANTS[usize::from(bx >= 4) + 2 * usize::from(by >= 2)]
    });
    let frame = decode(
        &encode_image(Image::Rgb, &texture, Compression::Snappy),
        width,
        height,
    );
    let pixels = convert(&gpu, &frame, width, height);

    assert_pixel(&pixels, width, 0, 0, [1.0, 0.0, 0.0, 1.0]);
    assert_pixel(&pixels, width, 31, 0, [0.0, 1.0, 0.0, 1.0]);
    assert_pixel(&pixels, width, 0, 15, [0.0, 0.0, 1.0, 1.0]);
    assert_pixel(&pixels, width, 31, 15, [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn odd_sizes_drop_the_block_padding() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (30, 18);
    let texture = plane_from_fn(BlockFormat::Bc1, width, height, |bx, by| {
        // Last block column and row hold padding pixels in another color.
        if bx == 7 || by == 4 {
            [0, 0, 255, 255]
        } else {
            [255, 0, 0, 255]
        }
    });
    let frame = decode(
        &encode_image(Image::Rgb, &texture, Compression::None),
        width,
        height,
    );
    let pixels = convert(&gpu, &frame, width, height);

    assert_eq!(pixels.len(), 30 * 18);
    assert_pixel(&pixels, width, 27, 15, [1.0, 0.0, 0.0, 1.0]);
    assert_pixel(&pixels, width, 29, 17, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn hap_alpha_is_premultiplied() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (16, 8);
    let texture = solid_plane(BlockFormat::Bc3, width, height, [255, 128, 0, 102]);
    let frame = decode(
        &encode_image(Image::Rgba, &texture, Compression::Snappy),
        width,
        height,
    );
    let pixels = convert(&gpu, &frame, width, height);

    let alpha = 102.0 / 255.0;
    let [r, g, b] = quantized_rgb([255, 128, 0]);
    assert_pixel(
        &pixels,
        width,
        5,
        5,
        [r * alpha, g * alpha, b * alpha, alpha],
    );
}

/// Expected RGB of a scaled-YCoCg block holding `rgb`, after RGB565
/// quantization of Co and Cg.
fn ycocg_expected(rgb: [u8; 3]) -> [f32; 3] {
    let [co, cg, _, y] = ycocg_color(rgb);
    let co = quantized(co, 5) - 128.0 / 255.0;
    let cg = quantized(cg, 6) - 128.0 / 255.0;
    let y = f32::from(y) / 255.0;
    [y + co - cg, y + cg, y - co - cg].map(|v| v.clamp(0.0, 1.0))
}

#[test]
fn hap_q_is_converted_from_ycocg() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (16, 8);
    for rgb in [[255, 0, 0], [40, 200, 90], [128, 128, 128]] {
        let texture = solid_plane(BlockFormat::Bc3, width, height, ycocg_color(rgb));
        let frame = decode(
            &encode_image(Image::YCoCg, &texture, Compression::Snappy),
            width,
            height,
        );
        let pixels = convert(&gpu, &frame, width, height);
        assert_pixel(&pixels, width, 3, 3, opaque(ycocg_expected(rgb)));
    }
}

#[test]
fn hap_q_alpha_uses_the_alpha_plane() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (16, 8);
    let color = solid_plane(
        BlockFormat::Bc3,
        width,
        height,
        ycocg_color([255, 255, 255]),
    );
    let alpha = solid_plane(BlockFormat::Bc4, width, height, [51, 0, 0, 0]);
    let data = encode_multiple(&[
        encode_image(Image::YCoCg, &color, Compression::Snappy),
        encode_image(Image::Alpha, &alpha, Compression::Snappy),
    ]);
    let pixels = convert(&gpu, &decode(&data, width, height), width, height);

    let a = 0.2;
    let [r, g, b] = ycocg_expected([255, 255, 255]);
    assert_pixel(&pixels, width, 8, 4, [r * a, g * a, b * a, a]);
}

#[test]
fn a_texture_follows_successive_frames() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (16, 8);
    let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
    let mut target = VideoTexture::new(&gpu.device, width, height);

    // Plain HAP, then HAP Q Alpha, then plain HAP again: formats and the
    // alpha plane change between frames.
    let red = encode_image(
        Image::Rgb,
        &solid_plane(BlockFormat::Bc1, width, height, [255, 0, 0, 255]),
        Compression::None,
    );
    let translucent = encode_multiple(&[
        encode_image(
            Image::YCoCg,
            &solid_plane(
                BlockFormat::Bc3,
                width,
                height,
                ycocg_color([255, 255, 255]),
            ),
            Compression::None,
        ),
        encode_image(
            Image::Alpha,
            &solid_plane(BlockFormat::Bc4, width, height, [0, 0, 0, 0]),
            Compression::None,
        ),
    ]);
    let expected = [
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
    ];
    for (data, expected) in [&red, &translucent, &red].into_iter().zip(expected) {
        let frame = decode(data, width, height);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        uploader
            .upload(&gpu.device, &gpu.queue, &mut encoder, &mut target, &frame)
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let pixels = read_texture(&gpu.device, &gpu.queue, target.texture()).unwrap();
        assert_pixel(&pixels, width, 1, 1, expected);
    }
}

#[test]
fn hap_file_plays_through_the_compositor() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (64u16, 32u16);
    // Frame i is a solid gray of level 64 * i.
    let frames: Vec<_> = (0..4u8)
        .map(|i| {
            let level = i * 64;
            let texture = solid_plane(
                BlockFormat::Bc1,
                width.into(),
                height.into(),
                [level, level, level, 255],
            );
            encode_image(Image::Rgb, &texture, Compression::Snappy)
        })
        .collect();
    let mut file = Vec::new();
    write_mov(
        &mut file,
        &MovOptions::hap(*b"Hap1", width, height, 60),
        &frames,
    )
    .unwrap();
    let mut decoder = HapDecoder::new(Cursor::new(file)).unwrap();

    let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
    let mut video = VideoTexture::new(&gpu.device, width.into(), height.into());
    let resolution = Resolution {
        width: 32,
        height: 16,
    };
    let mut compositor = Compositor::new(&gpu.device, resolution);
    let mut hap = HapFrame::default();

    for index in [2usize, 0, 3] {
        decoder.decode_frame(index, &mut hap).unwrap();
        let frame = Frame::Hap(hap.clone());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        uploader
            .upload(&gpu.device, &gpu.queue, &mut encoder, &mut video, &frame)
            .unwrap();
        gpu.queue.submit([encoder.finish()]);

        let layers = [LayerInput::new(video.view(), 0.5, BlendMode::Normal)];
        let output = compositor.render(&gpu.device, &gpu.queue, &layers);
        let pixels = read_texture(&gpu.device, &gpu.queue, output).unwrap();

        // Half opacity over opaque black.
        let [r, g, b] = quantized_rgb([(index * 64) as u8; 3]);
        assert_pixel(&pixels, 32, 0, 0, [r * 0.5, g * 0.5, b * 0.5, 1.0]);
    }
}

#[test]
fn wrong_plane_size_is_reported() {
    let Some(gpu) = gpu() else { return };
    let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
    let mut target = VideoTexture::new(&gpu.device, 16, 8);
    let mut frame = decode(
        &encode_image(
            Image::Rgb,
            &solid_plane(BlockFormat::Bc1, 16, 8, [0; 4]),
            Compression::None,
        ),
        16,
        8,
    );
    if let Frame::Hap(hap) = &mut frame {
        hap.color.data.truncate(8);
    }
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let error = uploader
        .upload(&gpu.device, &gpu.queue, &mut encoder, &mut target, &frame)
        .unwrap_err();
    assert!(matches!(
        error,
        prism_render::UploadError::WrongSize {
            expected: 64,
            actual: 8
        }
    ));
}

#[test]
fn rgba_frames_are_premultiplied_and_follow_hap_frames() {
    let Some(gpu) = gpu() else { return };
    let (width, height) = (4, 2);
    // Straight alpha: red at 40 %, then opaque blue.
    let mut pixels = Vec::new();
    for x in 0..8 {
        pixels.extend_from_slice(if x % 4 < 2 {
            &[255, 0, 0, 102]
        } else {
            &[0, 0, 255, 255]
        });
    }
    let image = Frame::Rgba(RgbaImage {
        width,
        height,
        pixels,
    });
    let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
    let mut target = VideoTexture::new(&gpu.device, width, height);
    let hap = decode(
        &encode_image(
            Image::Rgb,
            &solid_plane(BlockFormat::Bc1, width, height, [0, 255, 0, 255]),
            Compression::None,
        ),
        width,
        height,
    );
    // The same texture shows a HAP frame, then pixels, then HAP again.
    for (frame, expected) in [
        (&hap, [0.0, 1.0, 0.0, 1.0]),
        (&image, [0.4, 0.0, 0.0, 0.4]),
        (&hap, [0.0, 1.0, 0.0, 1.0]),
    ] {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        uploader
            .upload(&gpu.device, &gpu.queue, &mut encoder, &mut target, frame)
            .unwrap();
        gpu.queue.submit([encoder.finish()]);
        let pixels = read_texture(&gpu.device, &gpu.queue, target.texture()).unwrap();
        assert_pixel(&pixels, width, 0, 1, expected);
    }
    let pixels = read_texture(&gpu.device, &gpu.queue, target.texture()).unwrap();
    assert_pixel(&pixels, width, 3, 0, [0.0, 1.0, 0.0, 1.0]);

    // Pixels of the wrong size are refused.
    let small = Frame::Rgba(RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![0; 4],
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    assert!(matches!(
        uploader.upload(&gpu.device, &gpu.queue, &mut encoder, &mut target, &small),
        Err(prism_render::UploadError::WrongSize {
            expected: 32,
            actual: 4
        })
    ));
}
