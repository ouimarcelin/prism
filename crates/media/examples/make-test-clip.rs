//! Writes a HAP test clip of scrolling color bars, for manual tests without
//! your own media.
//!
//! ```text
//! cargo run -p prism-media --features test-support --example make-test-clip -- \
//!     media/bars.mov [hap|hap-alpha|hap-q|hap-q-alpha] [frames] [width] [height]
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;

use prism_media::BlockFormat;
use prism_media::test_support::{
    Compression, Image, MovOptions, encode_image, encode_multiple, plane_from_fn, write_mov,
    ycocg_color,
};

const BARS: [[u8; 3]; 8] = [
    [255, 255, 255],
    [255, 255, 0],
    [0, 255, 255],
    [0, 255, 0],
    [255, 0, 255],
    [255, 0, 0],
    [0, 0, 255],
    [16, 16, 16],
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!(
            "usage: make-test-clip OUTPUT.mov [hap|hap-alpha|hap-q|hap-q-alpha] [frames] [width] [height]"
        );
        return ExitCode::FAILURE;
    };
    let variant = args.get(1).map_or("hap", String::as_str);
    let number = |index: usize, default: u32| {
        args.get(index)
            .and_then(|a| a.parse().ok())
            .unwrap_or(default)
    };
    let frames = number(2, 240) as usize;
    let width = number(3, 1920);
    let height = number(4, 1080);

    let codec = match variant {
        "hap" => *b"Hap1",
        "hap-alpha" => *b"Hap5",
        "hap-q" => *b"HapY",
        "hap-q-alpha" => *b"HapM",
        other => {
            eprintln!("unknown variant {other}");
            return ExitCode::FAILURE;
        }
    };

    let blocks_wide = width.div_ceil(4);
    let bar_of = move |bx: u32, frame: usize| {
        let shifted = (bx as usize + frame * 2) % blocks_wide as usize;
        BARS[shifted * BARS.len() / blocks_wide as usize]
    };
    // Alpha ramps from transparent at the top to opaque at the bottom.
    let alpha_of =
        move |by: u32| (by * 255 / height.div_ceil(4).max(2).saturating_sub(1)).min(255) as u8;

    let encoded: Vec<Vec<u8>> = (0..frames)
        .map(|frame| match variant {
            "hap" => {
                let texture = plane_from_fn(BlockFormat::Bc1, width, height, |bx, _| {
                    let [r, g, b] = bar_of(bx, frame);
                    [r, g, b, 255]
                });
                encode_image(
                    Image::Rgb,
                    &texture,
                    Compression::Chunks {
                        count: 8,
                        offsets: false,
                    },
                )
            }
            "hap-alpha" => {
                let texture = plane_from_fn(BlockFormat::Bc3, width, height, |bx, by| {
                    let [r, g, b] = bar_of(bx, frame);
                    [r, g, b, alpha_of(by)]
                });
                encode_image(Image::Rgba, &texture, Compression::Snappy)
            }
            _ => {
                let color = plane_from_fn(BlockFormat::Bc3, width, height, |bx, _| {
                    ycocg_color(bar_of(bx, frame))
                });
                let color = encode_image(Image::YCoCg, &color, Compression::Snappy);
                if variant == "hap-q" {
                    color
                } else {
                    let alpha = plane_from_fn(BlockFormat::Bc4, width, height, |_, by| {
                        [alpha_of(by), 0, 0, 0]
                    });
                    encode_multiple(&[
                        color,
                        encode_image(Image::Alpha, &alpha, Compression::Snappy),
                    ])
                }
            }
        })
        .collect();

    let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
        eprintln!("size too large");
        return ExitCode::FAILURE;
    };
    let result = File::create(path).and_then(|file| {
        write_mov(
            &mut BufWriter::new(file),
            &MovOptions::hap(codec, w, h, 60),
            &encoded,
        )
    });
    match result {
        Ok(()) => {
            println!("wrote {path}: {variant}, {frames} frames, {width}x{height} at 60 fps");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cannot write {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
