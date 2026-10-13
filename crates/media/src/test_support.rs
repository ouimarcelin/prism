//! Writers for HAP frames and QuickTime files, for tests.
//!
//! Enabled by the `test-support` feature. Files are valid but minimal: they
//! hold only the atoms needed to describe one video track.

use std::io::{self, Write};

use crate::hap::BlockFormat;
use crate::mov::FourCc;

/// Packs an 8-bit color into RGB565, as stored in BC1 endpoints.
pub fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    (u16::from(r >> 3) << 11) | (u16::from(g >> 2) << 5) | u16::from(b >> 3)
}

/// Color of a scaled-YCoCg block (scale 1) encoding an 8-bit RGB color:
/// Co in red, Cg in green, scale index 0 in blue, Y in alpha, chroma offset
/// by 128 as the HAP Q decoding expects.
pub fn ycocg_color(rgb: [u8; 3]) -> [u8; 4] {
    let [r, g, b] = rgb.map(f32::from);
    let y = r / 4.0 + g / 2.0 + b / 4.0;
    let co = r / 2.0 - b / 2.0 + 128.0;
    let cg = -r / 4.0 + g / 2.0 - b / 4.0 + 128.0;
    let byte = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [byte(co), byte(cg), 0, byte(y)]
}

/// One block of a single color. BC1 uses RGB, BC3 RGBA, BC4 the red value.
///
/// # Panics
///
/// Panics for BC7, which this writer does not encode.
pub fn solid_block(format: BlockFormat, rgba: [u8; 4]) -> Vec<u8> {
    let endpoint = rgb565(rgba[0], rgba[1], rgba[2]).to_le_bytes();
    // Both endpoints equal and every index 0: all 16 pixels take endpoint 0.
    let bc1 = [
        endpoint[0],
        endpoint[1],
        endpoint[0],
        endpoint[1],
        0,
        0,
        0,
        0,
    ];
    match format {
        BlockFormat::Bc1 => bc1.to_vec(),
        BlockFormat::Bc3 => [rgba[3], rgba[3], 0, 0, 0, 0, 0, 0]
            .into_iter()
            .chain(bc1)
            .collect(),
        BlockFormat::Bc4 => vec![rgba[0], rgba[0], 0, 0, 0, 0, 0, 0],
        BlockFormat::Bc7 => panic!("BC7 blocks are not supported by the test writer"),
    }
}

/// A texture whose block at column `bx`, row `by` has the color `color(bx, by)`.
pub fn plane_from_fn(
    format: BlockFormat,
    width: u32,
    height: u32,
    color: impl Fn(u32, u32) -> [u8; 4],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(format.image_bytes(width, height));
    for by in 0..height.div_ceil(4) {
        for bx in 0..width.div_ceil(4) {
            data.extend(solid_block(format, color(bx, by)));
        }
    }
    data
}

/// A texture of a single color.
pub fn solid_plane(format: BlockFormat, width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    plane_from_fn(format, width, height, |_, _| rgba)
}

/// Image format of a top-level section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Image {
    /// BC1 RGB.
    Rgb,
    /// BC3 RGBA.
    Rgba,
    /// BC3 scaled YCoCg.
    YCoCg,
    /// BC4 alpha.
    Alpha,
}

impl Image {
    fn nibble(self) -> u8 {
        match self {
            Self::Rgb => 0xB,
            Self::Rgba => 0xE,
            Self::YCoCg => 0xF,
            Self::Alpha => 0x1,
        }
    }
}

/// Second-stage compression applied by [`encode_image`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// Stored as is.
    None,
    /// One Snappy stream.
    Snappy,
    /// Split into chunks, alternately Snappy and uncompressed. With
    /// `offsets`, an offset table is written, the chunks are stored in
    /// reverse order and headers use the 8-byte form.
    Chunks {
        /// Number of chunks.
        count: usize,
        /// Whether to write a chunk offset table.
        offsets: bool,
    },
}

/// Appends a section header and its content.
pub fn write_section(out: &mut Vec<u8>, kind: u8, body: &[u8], long_header: bool) {
    let size = u32::try_from(body.len()).expect("section larger than 4 GiB");
    if long_header || size == 0 || size >= 1 << 24 {
        out.extend([0, 0, 0, kind]);
        out.extend(size.to_le_bytes());
    } else {
        out.extend(&size.to_le_bytes()[..3]);
        out.push(kind);
    }
    out.extend_from_slice(body);
}

fn compress(data: &[u8]) -> Vec<u8> {
    snap::raw::Encoder::new()
        .compress_vec(data)
        .expect("Snappy compression of test data")
}

/// Encodes a top-level image section.
pub fn encode_image(image: Image, texture: &[u8], compression: Compression) -> Vec<u8> {
    let mut out = Vec::new();
    match compression {
        Compression::None => write_section(&mut out, 0xA0 | image.nibble(), texture, false),
        Compression::Snappy => {
            write_section(&mut out, 0xB0 | image.nibble(), &compress(texture), false);
        }
        Compression::Chunks { count, offsets } => {
            let chunk_size = texture.len().div_ceil(count.max(1)).max(1);
            let chunks: Vec<(u8, Vec<u8>)> = texture
                .chunks(chunk_size)
                .enumerate()
                .map(|(i, chunk)| {
                    if i % 2 == 0 {
                        (0x0B, compress(chunk))
                    } else {
                        (0x0A, chunk.to_vec())
                    }
                })
                .collect();

            let mut frame_data: Vec<u8> = Vec::new();
            let mut positions = vec![0u32; chunks.len()];
            let order: Vec<usize> = if offsets {
                (0..chunks.len()).rev().collect()
            } else {
                (0..chunks.len()).collect()
            };
            for index in order {
                positions[index] = u32::try_from(frame_data.len()).expect("test frame too large");
                frame_data.extend(&chunks[index].1);
            }

            let compressors: Vec<u8> = chunks.iter().map(|(c, _)| *c).collect();
            let sizes: Vec<u8> = chunks
                .iter()
                .flat_map(|(_, data)| {
                    u32::try_from(data.len())
                        .expect("chunk too large")
                        .to_le_bytes()
                })
                .collect();
            let mut tables = Vec::new();
            write_section(&mut tables, 0x02, &compressors, offsets);
            write_section(&mut tables, 0x03, &sizes, offsets);
            if offsets {
                let offset_table: Vec<u8> =
                    positions.iter().flat_map(|p| p.to_le_bytes()).collect();
                write_section(&mut tables, 0x04, &offset_table, true);
            }

            let mut body = Vec::new();
            write_section(&mut body, 0x01, &tables, offsets);
            body.extend(frame_data);
            write_section(&mut out, 0xC0 | image.nibble(), &body, offsets);
        }
    }
    out
}

/// Wraps encoded image sections in a multiple-images section.
pub fn encode_multiple(images: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    write_section(&mut out, 0x0D, &images.concat(), false);
    out
}

/// Description of the file written by [`write_mov`].
#[derive(Clone, Copy, Debug)]
pub struct MovOptions {
    /// Codec of the frames.
    pub codec: FourCc,
    /// Frame width.
    pub width: u16,
    /// Frame height.
    pub height: u16,
    /// Time units per second.
    pub timescale: u32,
    /// Duration of every frame, in time units.
    pub frame_duration: u32,
    /// Frames stored in each chunk.
    pub frames_per_chunk: usize,
    /// Whether to write 64-bit chunk offsets (`co64`) instead of `stco`.
    pub large_offsets: bool,
}

impl MovOptions {
    /// HAP at 1/`fps` seconds per frame, 2 frames per chunk.
    pub fn hap(codec: FourCc, width: u16, height: u16, fps: u32) -> Self {
        Self {
            codec,
            width,
            height,
            timescale: fps * 100,
            frame_duration: 100,
            frames_per_chunk: 2,
            large_offsets: false,
        }
    }
}

fn atom(kind: &[u8; 4], parts: &[&[u8]]) -> Vec<u8> {
    let size: usize = 8 + parts.iter().map(|part| part.len()).sum::<usize>();
    let mut out = Vec::with_capacity(size);
    out.extend(u32::try_from(size).expect("atom too large").to_be_bytes());
    out.extend(kind);
    for part in parts {
        out.extend_from_slice(part);
    }
    out
}

fn be32(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

const IDENTITY_MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

/// Writes a QuickTime file holding `frames` as its only video track.
pub fn write_mov<W: Write>(
    out: &mut W,
    options: &MovOptions,
    frames: &[Vec<u8>],
) -> io::Result<()> {
    let count = u32::try_from(frames.len()).expect("too many frames");
    let duration = count * options.frame_duration;

    let file_type = atom(b"ftyp", &[b"qt  ", &[0, 0, 0, 0], b"qt  "]);
    let media_data = frames.concat();
    let mut offset = (file_type.len() + 8) as u64;
    let mut chunk_offsets = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        if index % options.frames_per_chunk == 0 {
            chunk_offsets.push(offset);
        }
        offset += frame.len() as u64;
    }

    let movie_header = atom(
        b"mvhd",
        &[
            &be32(&[0, 0, 0, options.timescale, duration, 0x0001_0000]),
            &[0x01, 0x00],
            &[0; 10],
            &be32(&IDENTITY_MATRIX),
            &be32(&[0, 0, 0, 0, 0, 0, 2]),
        ],
    );
    let track_header = atom(
        b"tkhd",
        &[
            &be32(&[3, 0, 0, 1, 0, duration, 0, 0]),
            &[0; 8],
            &be32(&IDENTITY_MATRIX),
            &be32(&[
                u32::from(options.width) << 16,
                u32::from(options.height) << 16,
            ]),
        ],
    );
    let media_header = atom(
        b"mdhd",
        &[&be32(&[0, 0, 0, options.timescale, duration]), &[0; 4]],
    );
    let handler = atom(b"hdlr", &[&[0; 4], b"mhlr", b"vide", &[0; 12], &[0]]);
    let video_header = atom(b"vmhd", &[&[0, 0, 0, 1], &[0; 8]]);
    let data_info = atom(
        b"dinf",
        &[&atom(
            b"dref",
            &[&be32(&[0, 1]), &atom(b"url ", &[&[0, 0, 0, 1]])],
        )],
    );

    let mut description = Vec::new();
    description.extend([0; 6]);
    description.extend(1u16.to_be_bytes());
    description.extend([0; 16]);
    description.extend(options.width.to_be_bytes());
    description.extend(options.height.to_be_bytes());
    description.extend(be32(&[0x0048_0000, 0x0048_0000, 0]));
    description.extend(1u16.to_be_bytes());
    description.extend([0; 32]);
    description.extend(24u16.to_be_bytes());
    description.extend((-1i16).to_be_bytes());
    let sample_description = atom(
        b"stsd",
        &[&be32(&[0, 1]), &atom(&options.codec, &[&description])],
    );

    let time_to_sample = atom(b"stts", &[&be32(&[0, 1, count, options.frame_duration])]);
    let per_chunk = u32::try_from(options.frames_per_chunk).expect("chunk too large");
    let full_chunks = count / per_chunk;
    let remainder = count % per_chunk;
    let mut chunk_rules = vec![1, per_chunk, 1];
    if remainder != 0 && full_chunks > 0 {
        chunk_rules.extend([full_chunks + 1, remainder, 1]);
    } else if remainder != 0 {
        chunk_rules = vec![1, remainder, 1];
    }
    let rule_count = u32::try_from(chunk_rules.len() / 3).expect("rule count");
    let sample_to_chunk = atom(b"stsc", &[&be32(&[0, rule_count]), &be32(&chunk_rules)]);
    let sizes: Vec<u32> = frames
        .iter()
        .map(|frame| u32::try_from(frame.len()).expect("frame too large"))
        .collect();
    let sample_sizes = atom(b"stsz", &[&be32(&[0, 0, count]), &be32(&sizes)]);
    let chunk_count = u32::try_from(chunk_offsets.len()).expect("chunk count");
    let chunk_offset_atom = if options.large_offsets {
        let offsets: Vec<u8> = chunk_offsets.iter().flat_map(|o| o.to_be_bytes()).collect();
        atom(b"co64", &[&be32(&[0, chunk_count]), &offsets])
    } else {
        let offsets: Vec<u32> = chunk_offsets
            .iter()
            .map(|&o| u32::try_from(o).expect("offset too large for stco"))
            .collect();
        atom(b"stco", &[&be32(&[0, chunk_count]), &be32(&offsets)])
    };

    let sample_table = atom(
        b"stbl",
        &[
            &sample_description,
            &time_to_sample,
            &sample_to_chunk,
            &sample_sizes,
            &chunk_offset_atom,
        ],
    );
    let media_info = atom(b"minf", &[&video_header, &data_info, &sample_table]);
    let media = atom(b"mdia", &[&media_header, &handler, &media_info]);
    let track = atom(b"trak", &[&track_header, &media]);
    let movie = atom(b"moov", &[&movie_header, &track]);

    out.write_all(&file_type)?;
    out.write_all(&atom(b"mdat", &[&media_data]))?;
    out.write_all(&movie)?;
    Ok(())
}
