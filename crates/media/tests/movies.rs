//! H.264 and ProRes movies decoded by AVFoundation (macOS only).
//!
//! The fixtures are 30 frames at 30 per second, 64 × 32: ten red, ten green,
//! ten blue. They were made from synthetic images (see docs/SOURCES.md).

#![cfg(target_os = "macos")]
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use prism_media::{ChannelOrder, Frame, FrameSource, open_file};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Center pixel as red, green, blue, alpha.
fn center(frame: &Frame) -> [u8; 4] {
    if let Frame::Yuv(image) = frame {
        assert_eq!((image.width, image.height), (64, 32));
        let [r, g, b] = image.rgb_at(32, 16);
        return [r, g, b, 255];
    }
    let Frame::Rgba(image) = frame else {
        panic!("expected pixels")
    };
    assert_eq!((image.width, image.height), (64, 32));
    let at = (16 * 64 + 32) * 4;
    let p = &image.pixels[at..at + 4];
    match image.order {
        ChannelOrder::Rgba => [p[0], p[1], p[2], p[3]],
        ChannelOrder::Bgra => [p[2], p[1], p[0], p[3]],
    }
}

/// Which third a color belongs to: 0 red, 1 green, 2 blue.
fn third(rgba: [u8; 4]) -> usize {
    let [r, g, b, _] = rgba.map(u16::from);
    if r > g && r > b {
        0
    } else if g > b {
        1
    } else {
        2
    }
}

fn check_thirds(source: &mut dyn FrameSource) {
    let mut frame = Frame::default();
    // Forward reads, a jump back, a jump ahead, then the last frame.
    for index in [0, 1, 2, 15, 16, 25, 5, 29, 12] {
        source.decode(index, &mut frame).unwrap();
        assert_eq!(
            third(center(&frame)),
            index / 10,
            "frame {index}: {:?}",
            center(&frame)
        );
    }
    assert!(source.decode(30, &mut frame).is_err());
}

#[test]
fn h264_with_b_frames_decodes_any_frame() {
    let mut source = open_file(&fixture("rgb-thirds-h264.mp4")).unwrap();
    let info = source.info().clone();
    assert_eq!(info.format, "avc1");
    assert_eq!((info.width, info.height, info.frame_count()), (64, 32, 30));
    assert!((info.frame_rate() - 30.0).abs() < 1e-6);
    check_thirds(source.as_mut());
}

#[test]
fn prores_decodes_any_frame() {
    let mut source = open_file(&fixture("rgb-thirds-prores.mov")).unwrap();
    assert_eq!(source.info().format, "apcn");
    check_thirds(source.as_mut());
}

#[test]
fn prores_4444_alpha_is_straight() {
    // Red at half alpha.
    let mut source = open_file(&fixture("red-half-alpha-prores4444.mov")).unwrap();
    assert_eq!(source.info().format, "ap4h");
    let mut frame = Frame::default();
    source.decode(1, &mut frame).unwrap();
    let [r, g, b, a] = center(&frame);
    assert!((i16::from(a) - 128).abs() <= 3, "alpha {a}");
    // Straight alpha keeps full red; premultiplied would halve it.
    assert!(r >= 245 && g <= 10 && b <= 10, "color {:?}", [r, g, b]);
}
