//! Still images and PNG sequences.

#![allow(clippy::unwrap_used)]

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use prism_media::test_support::{
    Compression, Image, MovOptions, encode_image, solid_plane, write_mov,
};
use prism_media::{BlockFormat, DecodeError, Frame, open_file, open_sequence};

/// Writes a PNG of `width` × `height` from raw samples.
fn write_png(
    path: &Path,
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &[u8],
) {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path).unwrap()), width, height);
    encoder.set_color(color);
    encoder.set_depth(depth);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(data)
        .unwrap();
}

/// A 2 × 1 RGBA PNG: one red pixel at half alpha, one opaque green pixel,
/// then `extra` to tell sequence frames apart.
fn rgba_png(path: &Path, extra: u8) {
    write_png(
        path,
        2,
        1,
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        &[255, 0, extra, 128, 0, 255, 0, 255],
    );
}

fn pixels(frame: &Frame) -> &[u8] {
    match frame {
        Frame::Rgba(image) => &image.pixels,
        _ => panic!("expected an RGBA frame"),
    }
}

#[test]
fn still_png_is_a_one_frame_clip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logo.png");
    rgba_png(&path, 0);
    let mut source = open_file(&path).unwrap();
    let info = source.info().clone();
    assert_eq!(info.format, "PNG");
    assert_eq!((info.width, info.height, info.frame_count()), (2, 1, 1));
    assert_eq!(info.frame_at(12.0), 0);

    let mut frame = Frame::default();
    source.decode(0, &mut frame).unwrap();
    assert_eq!(pixels(&frame), [255, 0, 0, 128, 0, 255, 0, 255]);
    assert!(matches!(
        source.decode(1, &mut frame),
        Err(DecodeError::FrameOutOfRange { index: 1, count: 1 })
    ));
}

#[test]
fn every_png_color_type_becomes_rgba() {
    let dir = tempfile::tempdir().unwrap();
    let cases: [(png::ColorType, png::BitDepth, Vec<u8>, [u8; 4]); 4] = [
        (
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            vec![77],
            [77, 77, 77, 255],
        ),
        (
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            vec![77, 9],
            [77, 77, 77, 9],
        ),
        (
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            vec![1, 2, 3],
            [1, 2, 3, 255],
        ),
        // 16 bits per channel keep their high byte.
        (
            png::ColorType::Rgb,
            png::BitDepth::Sixteen,
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc],
            [0x12, 0x56, 0x9a, 255],
        ),
    ];
    for (index, (color, depth, data, expected)) in cases.into_iter().enumerate() {
        let path = dir.path().join(format!("{index}.png"));
        write_png(&path, 1, 1, color, depth, &data);
        let mut frame = Frame::default();
        open_file(&path).unwrap().decode(0, &mut frame).unwrap();
        assert_eq!(pixels(&frame), expected, "case {index}");
    }
}

#[test]
fn jpeg_still_decodes_close_to_its_colors() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/halves-16x8.jpg");
    let mut source = open_file(&path).unwrap();
    assert_eq!(source.info().format, "JPG");
    assert_eq!((source.info().width, source.info().height), (16, 8));
    let mut frame = Frame::default();
    source.decode(0, &mut frame).unwrap();
    let data = pixels(&frame);
    let at = |x: usize, y: usize| &data[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
    let close = |actual: &[u8], expected: [u8; 4]| {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (i16::from(*a) - i16::from(e)).abs() <= 12)
    };
    assert!(close(at(2, 4), [230, 20, 20, 255]), "left {:?}", at(2, 4));
    assert!(
        close(at(13, 4), [20, 20, 230, 255]),
        "right {:?}",
        at(13, 4)
    );
}

#[test]
fn sequence_plays_png_files_in_number_order() {
    let dir = tempfile::tempdir().unwrap();
    for (name, extra) in [("shot_10.png", 10), ("shot_2.png", 2), ("shot_1.png", 1)] {
        rgba_png(&dir.path().join(name), extra);
    }
    std::fs::write(dir.path().join("notes.txt"), b"ignored").unwrap();
    std::fs::write(dir.path().join(".hidden.png"), b"ignored").unwrap();

    let mut source = open_sequence(dir.path(), 25.0).unwrap();
    let info = source.info().clone();
    assert_eq!(info.frame_count(), 3);
    assert!((info.duration_seconds() - 0.12).abs() < 1e-12);
    assert_eq!(info.frame_at(0.05), 1);

    let mut frame = Frame::default();
    for (index, extra) in [(2, 10), (0, 1), (1, 2)] {
        source.decode(index, &mut frame).unwrap();
        assert_eq!(pixels(&frame)[2], extra, "frame {index}");
    }
}

#[test]
fn sequence_problems_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        open_sequence(dir.path(), 30.0),
        Err(DecodeError::Image(_))
    ));

    rgba_png(&dir.path().join("a1.png"), 0);
    write_png(
        &dir.path().join("a2.png"),
        3,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[0; 9],
    );
    let mut source = open_sequence(dir.path(), 30.0).unwrap();
    let mut frame = Frame::default();
    let error = source.decode(1, &mut frame).unwrap_err();
    assert!(
        matches!(&error, DecodeError::Image(message) if message.contains("3 × 1")),
        "{error}"
    );
}

#[test]
fn movies_still_open_as_hap() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mov");
    let texture = solid_plane(BlockFormat::Bc1, 8, 4, [255, 0, 0, 255]);
    let frame = encode_image(Image::Rgb, &texture, Compression::Snappy);
    write_mov(
        &mut File::create(&path).unwrap(),
        &MovOptions::hap(*b"Hap1", 8, 4, 30),
        &[frame],
    )
    .unwrap();
    let mut source = open_file(&path).unwrap();
    assert_eq!(source.info().format, "Hap1");
    let mut decoded = Frame::default();
    source.decode(0, &mut decoded).unwrap();
    assert!(matches!(decoded, Frame::Hap(hap) if hap.color.data == texture));
}

#[test]
fn unreadable_image_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.png");
    std::fs::write(&path, b"\x89PNG broken").unwrap();
    assert!(matches!(open_file(&path), Err(DecodeError::Image(_))));
}
