//! HAP files written to disk, read back frame by frame.

#![allow(clippy::unwrap_used)]

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use prism_media::test_support::{
    Compression, Image, MovOptions, encode_image, encode_multiple, plane_from_fn, solid_plane,
    write_mov,
};
use prism_media::{
    BlockFormat, ColorLayout, ContainerError, DecodeError, HapDecoder, HapError, HapFrame,
};

const WIDTH: u16 = 64;
const HEIGHT: u16 = 32;

/// A distinct texture for each frame index.
fn frame_texture(format: BlockFormat, index: usize) -> Vec<u8> {
    let shade = (index * 40 % 256) as u8;
    plane_from_fn(format, WIDTH.into(), HEIGHT.into(), |bx, by| {
        [shade, (bx * 16) as u8, (by * 32) as u8, 200]
    })
}

fn write_file(dir: &Path, options: &MovOptions, frames: &[Vec<u8>]) -> std::path::PathBuf {
    let path = dir.join("clip.mov");
    write_mov(&mut File::create(&path).unwrap(), options, frames).unwrap();
    path
}

#[test]
fn reads_track_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let frames: Vec<_> = (0..5)
        .map(|i| {
            encode_image(
                Image::Rgb,
                &frame_texture(BlockFormat::Bc1, i),
                Compression::Snappy,
            )
        })
        .collect();
    let path = write_file(
        dir.path(),
        &MovOptions::hap(*b"Hap1", WIDTH, HEIGHT, 30),
        &frames,
    );

    let decoder = HapDecoder::open(&path).unwrap();
    let track = decoder.track();
    assert_eq!(track.codec_name(), "Hap1");
    assert_eq!((track.width, track.height), (64, 32));
    assert_eq!(track.frame_count(), 5);
    assert!((track.frame_rate() - 30.0).abs() < 1e-9);
    assert!((track.duration_seconds() - 5.0 / 30.0).abs() < 1e-9);
    assert_eq!(track.frame_at(0.0), 0);
    assert_eq!(track.frame_at(0.05), 1);
    assert_eq!(track.frame_at(10.0), 4);
}

/// Writes frames with every compression and checks each decodes to the
/// exact texture, in random order.
fn round_trip(image: Image, format: BlockFormat, layout: ColorLayout, options: MovOptions) {
    let dir = tempfile::tempdir().unwrap();
    let compressions = [
        Compression::None,
        Compression::Snappy,
        Compression::Chunks {
            count: 3,
            offsets: false,
        },
        Compression::Chunks {
            count: 4,
            offsets: true,
        },
        Compression::Snappy,
    ];
    let textures: Vec<_> = (0..compressions.len())
        .map(|i| frame_texture(format, i))
        .collect();
    let frames: Vec<_> = textures
        .iter()
        .zip(compressions)
        .map(|(texture, compression)| encode_image(image, texture, compression))
        .collect();
    let path = write_file(dir.path(), &options, &frames);

    let mut decoder = HapDecoder::open(&path).unwrap();
    let mut frame = HapFrame::default();
    for index in [3, 0, 4, 1, 2, 2, 0] {
        decoder.decode_frame(index, &mut frame).unwrap();
        assert_eq!(frame.layout, layout);
        assert_eq!(frame.color.format, format);
        assert!(frame.alpha.is_none());
        assert_eq!(frame.color.data, textures[index], "frame {index}");
    }
}

#[test]
fn hap_frames_round_trip() {
    let options = MovOptions::hap(*b"Hap1", WIDTH, HEIGHT, 60);
    round_trip(Image::Rgb, BlockFormat::Bc1, ColorLayout::Rgb, options);
}

#[test]
fn hap_alpha_frames_round_trip() {
    let options = MovOptions::hap(*b"Hap5", WIDTH, HEIGHT, 60);
    round_trip(Image::Rgba, BlockFormat::Bc3, ColorLayout::Rgba, options);
}

#[test]
fn hap_q_frames_round_trip_with_64_bit_offsets_and_odd_chunks() {
    let options = MovOptions {
        large_offsets: true,
        frames_per_chunk: 3,
        ..MovOptions::hap(*b"HapY", WIDTH, HEIGHT, 25)
    };
    round_trip(
        Image::YCoCg,
        BlockFormat::Bc3,
        ColorLayout::ScaledYCoCg,
        options,
    );
}

#[test]
fn hap_q_alpha_has_two_planes_in_any_order() {
    let dir = tempfile::tempdir().unwrap();
    let color = frame_texture(BlockFormat::Bc3, 1);
    let alpha = solid_plane(BlockFormat::Bc4, WIDTH.into(), HEIGHT.into(), [77, 0, 0, 0]);
    let color_section = encode_image(Image::YCoCg, &color, Compression::Snappy);
    let alpha_section = encode_image(Image::Alpha, &alpha, Compression::None);
    let frames = vec![
        encode_multiple(&[color_section.clone(), alpha_section.clone()]),
        encode_multiple(&[alpha_section, color_section]),
    ];
    let path = write_file(
        dir.path(),
        &MovOptions::hap(*b"HapM", WIDTH, HEIGHT, 60),
        &frames,
    );

    let mut decoder = HapDecoder::open(&path).unwrap();
    let mut frame = HapFrame::default();
    for index in 0..2 {
        decoder.decode_frame(index, &mut frame).unwrap();
        assert_eq!(frame.layout, ColorLayout::ScaledYCoCg);
        assert_eq!(frame.color.data, color);
        let alpha_plane = frame.alpha.as_ref().unwrap();
        assert_eq!(alpha_plane.format, BlockFormat::Bc4);
        assert_eq!(alpha_plane.data, alpha);
    }

    // The alpha plane is dropped when the next frame has none.
    let single = encode_image(
        Image::Rgb,
        &frame_texture(BlockFormat::Bc1, 0),
        Compression::None,
    );
    prism_media::hap::decode_frame(&single, WIDTH.into(), HEIGHT.into(), &mut frame).unwrap();
    assert!(frame.alpha.is_none());
}

#[test]
fn odd_frame_size_uses_padded_blocks() {
    let (width, height) = (30u16, 18u16);
    let texture = solid_plane(
        BlockFormat::Bc1,
        width.into(),
        height.into(),
        [255, 0, 0, 255],
    );
    assert_eq!(texture.len(), 8 * 5 * 8);
    let frame = encode_image(Image::Rgb, &texture, Compression::Snappy);
    let mut out = Vec::new();
    write_mov(
        &mut out,
        &MovOptions::hap(*b"Hap1", width, height, 60),
        &[frame],
    )
    .unwrap();

    let mut decoder = HapDecoder::new(Cursor::new(out)).unwrap();
    let mut decoded = HapFrame::default();
    decoder.decode_frame(0, &mut decoded).unwrap();
    assert_eq!(decoded.color.data, texture);
}

#[test]
fn wrong_texture_size_is_rejected() {
    let texture = solid_plane(BlockFormat::Bc1, 8, 8, [0; 4]);
    let frame = encode_image(Image::Rgb, &texture, Compression::None);
    let mut out = Vec::new();
    write_mov(
        &mut out,
        &MovOptions::hap(*b"Hap1", WIDTH, HEIGHT, 60),
        &[frame],
    )
    .unwrap();

    let mut decoder = HapDecoder::new(Cursor::new(out)).unwrap();
    let error = decoder
        .decode_frame(0, &mut HapFrame::default())
        .unwrap_err();
    assert!(matches!(
        error,
        DecodeError::Hap(HapError::WrongSize {
            expected: 1024,
            actual: 32
        })
    ));
}

#[test]
fn frame_index_past_the_end_is_rejected() {
    let frame = encode_image(
        Image::Rgb,
        &frame_texture(BlockFormat::Bc1, 0),
        Compression::None,
    );
    let mut out = Vec::new();
    write_mov(
        &mut out,
        &MovOptions::hap(*b"Hap1", WIDTH, HEIGHT, 60),
        &[frame],
    )
    .unwrap();
    let mut decoder = HapDecoder::new(Cursor::new(out)).unwrap();
    assert!(matches!(
        decoder.decode_frame(1, &mut HapFrame::default()),
        Err(DecodeError::FrameOutOfRange { index: 1, count: 1 })
    ));
}

#[test]
fn other_codecs_are_refused() {
    let mut out = Vec::new();
    write_mov(
        &mut out,
        &MovOptions::hap(*b"avc1", WIDTH, HEIGHT, 60),
        &[vec![0; 16]],
    )
    .unwrap();
    assert!(matches!(
        HapDecoder::new(Cursor::new(out)),
        Err(DecodeError::NotHap(codec)) if codec == "avc1"
    ));
}

#[test]
fn truncated_file_is_reported() {
    let frame = encode_image(
        Image::Rgb,
        &frame_texture(BlockFormat::Bc1, 0),
        Compression::None,
    );
    let mut out = Vec::new();
    write_mov(
        &mut out,
        &MovOptions::hap(*b"Hap1", WIDTH, HEIGHT, 60),
        &[frame],
    )
    .unwrap();
    out.truncate(out.len() - 40);
    assert!(matches!(
        HapDecoder::new(Cursor::new(out)),
        Err(DecodeError::Container(
            ContainerError::Io(_) | ContainerError::Malformed(_)
        ))
    ));
}

#[test]
fn file_without_movie_atom_is_reported() {
    assert!(matches!(
        HapDecoder::new(Cursor::new(b"\0\0\0\x08free".to_vec())),
        Err(DecodeError::Container(ContainerError::Malformed(
            "no moov atom"
        )))
    ));
}
