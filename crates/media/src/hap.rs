//! Decoding of HAP frames into block-compressed textures for the GPU.
//!
//! Implements the public HAP frame specification (Vidvox, "Hap Video",
//! `documentation/HapVideoDRAFT.md`): sectioned layout, Snappy second-stage
//! compression, chunked decode instructions and multiple-image frames.
//! The result stays block-compressed (BC1, BC3, BC4 or BC7): the GPU samples
//! it directly.

/// Error raised while decoding a HAP frame.
#[derive(Debug, thiserror::Error)]
pub enum HapError {
    /// A section extends past the end of the frame.
    #[error("HAP frame is truncated")]
    Truncated,
    /// The section type is not a supported image format.
    #[error("unsupported HAP section type 0x{0:02X}")]
    UnsupportedFormat(u8),
    /// A chunk names an unknown second-stage compressor.
    #[error("unknown HAP second-stage compressor 0x{0:02X}")]
    UnknownCompressor(u8),
    /// The decode instructions are incomplete or inconsistent.
    #[error("invalid HAP decode instructions: {0}")]
    BadInstructions(&'static str),
    /// A multiple-image frame holds a combination the specification forbids.
    #[error("unsupported combination of images in a HAP frame")]
    BadCombination,
    /// Snappy decompression failed.
    #[error("Snappy decompression failed")]
    Snappy(#[from] snap::Error),
    /// The decoded texture does not match the frame size.
    #[error("HAP texture has {actual} bytes, expected {expected}")]
    WrongSize {
        /// Bytes expected for the frame size.
        expected: usize,
        /// Bytes decoded.
        actual: usize,
    },
}

/// Block compression format of a texture plane. Blocks are 4 × 4 pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlockFormat {
    /// DXT1: RGB, 8 bytes per block.
    #[default]
    Bc1,
    /// DXT5: RGBA, 16 bytes per block.
    Bc3,
    /// RGTC1: one channel, 8 bytes per block.
    Bc4,
    /// BPTC: RGBA, 16 bytes per block.
    Bc7,
}

impl BlockFormat {
    /// Bytes per 4 × 4 block.
    pub fn block_bytes(self) -> usize {
        match self {
            Self::Bc1 | Self::Bc4 => 8,
            Self::Bc3 | Self::Bc7 => 16,
        }
    }

    /// Bytes of a full image of `width` × `height` pixels.
    pub fn image_bytes(self, width: u32, height: u32) -> usize {
        width.div_ceil(4) as usize * height.div_ceil(4) as usize * self.block_bytes()
    }
}

/// Meaning of the channels of the color plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorLayout {
    /// RGB, opaque.
    #[default]
    Rgb,
    /// RGBA with straight (non-premultiplied) alpha.
    Rgba,
    /// Scaled YCoCg: Co in red, Cg in green, scale in blue, Y in alpha.
    ScaledYCoCg,
}

/// One block-compressed texture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plane {
    /// Compression format.
    pub format: BlockFormat,
    /// Blocks, row by row from the top.
    pub data: Vec<u8>,
}

/// A decoded HAP frame: a color plane and, for HAP Q Alpha, an alpha plane.
///
/// Reuse the same value across frames: buffers keep their capacity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HapFrame {
    /// Meaning of the color plane channels.
    pub layout: ColorLayout,
    /// Color texture.
    pub color: Plane,
    /// Separate alpha texture (BC4, red channel), for HAP Q Alpha.
    pub alpha: Option<Plane>,
}

const MULTIPLE_IMAGES: u8 = 0x0D;
const INSTRUCTIONS_CONTAINER: u8 = 0x01;
const COMPRESSOR_TABLE: u8 = 0x02;
const SIZE_TABLE: u8 = 0x03;
const OFFSET_TABLE: u8 = 0x04;
const CHUNK_UNCOMPRESSED: u8 = 0x0A;
const CHUNK_SNAPPY: u8 = 0x0B;

/// Image format named by the low nibble of a top-level section type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Image {
    Rgb,
    Rgba,
    YCoCg,
    Bptc,
    Alpha,
}

impl Image {
    fn from_section(kind: u8) -> Result<Self, HapError> {
        match kind & 0x0F {
            0xB => Ok(Self::Rgb),
            0xE => Ok(Self::Rgba),
            0xF => Ok(Self::YCoCg),
            0xC => Ok(Self::Bptc),
            0x1 => Ok(Self::Alpha),
            _ => Err(HapError::UnsupportedFormat(kind)),
        }
    }

    fn format(self) -> BlockFormat {
        match self {
            Self::Rgb => BlockFormat::Bc1,
            Self::Rgba | Self::YCoCg => BlockFormat::Bc3,
            Self::Bptc => BlockFormat::Bc7,
            Self::Alpha => BlockFormat::Bc4,
        }
    }

    fn layout(self) -> ColorLayout {
        match self {
            Self::Rgb => ColorLayout::Rgb,
            Self::Rgba | Self::Bptc | Self::Alpha => ColorLayout::Rgba,
            Self::YCoCg => ColorLayout::ScaledYCoCg,
        }
    }
}

/// A section: its type, its content and the bytes after it.
type Section<'a> = (u8, &'a [u8], &'a [u8]);

/// Splits the first section off `data`.
fn section(data: &[u8]) -> Result<Section<'_>, HapError> {
    if data.len() < 4 {
        return Err(HapError::Truncated);
    }
    let (size, header): (usize, usize) = if data[..3] == [0, 0, 0] {
        let bytes = data.get(4..8).ok_or(HapError::Truncated)?;
        let size = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        (size as usize, 8)
    } else {
        let size = u32::from_le_bytes([data[0], data[1], data[2], 0]);
        (size as usize, 4)
    };
    let end = header.checked_add(size).ok_or(HapError::Truncated)?;
    if end > data.len() {
        return Err(HapError::Truncated);
    }
    Ok((data[3], &data[header..end], &data[end..]))
}

/// Decodes a HAP frame of `width` × `height` pixels into `frame`.
pub fn decode_frame(
    data: &[u8],
    width: u32,
    height: u32,
    frame: &mut HapFrame,
) -> Result<(), HapError> {
    let (kind, body, _) = section(data)?;
    if kind != MULTIPLE_IMAGES {
        return decode_single(kind, body, width, height, frame);
    }

    let (first_kind, first, rest) = section(body)?;
    if rest.is_empty() {
        // A single contained image is treated as a standalone top-level section.
        return decode_single(first_kind, first, width, height, frame);
    }
    let (second_kind, second, _) = section(rest)?;
    let (color, alpha) = match (
        Image::from_section(first_kind)?,
        Image::from_section(second_kind)?,
    ) {
        (Image::YCoCg, Image::Alpha) => ((first_kind, first), (second_kind, second)),
        (Image::Alpha, Image::YCoCg) => ((second_kind, second), (first_kind, first)),
        _ => return Err(HapError::BadCombination),
    };
    decode_image(color.0, color.1, width, height, &mut frame.color)?;
    let alpha_plane = frame.alpha.get_or_insert_with(Plane::default);
    decode_image(alpha.0, alpha.1, width, height, alpha_plane)?;
    frame.layout = ColorLayout::ScaledYCoCg;
    Ok(())
}

/// Decodes a frame made of one color image.
fn decode_single(
    kind: u8,
    body: &[u8],
    width: u32,
    height: u32,
    frame: &mut HapFrame,
) -> Result<(), HapError> {
    let image = Image::from_section(kind)?;
    if image == Image::Alpha {
        // Alpha-only frames (HapA) carry no color.
        return Err(HapError::UnsupportedFormat(kind));
    }
    decode_image(kind, body, width, height, &mut frame.color)?;
    frame.layout = image.layout();
    frame.alpha = None;
    Ok(())
}

/// Decodes one simple top-level section into `plane`.
fn decode_image(
    kind: u8,
    body: &[u8],
    width: u32,
    height: u32,
    plane: &mut Plane,
) -> Result<(), HapError> {
    let format = Image::from_section(kind)?.format();
    plane.format = format;
    match kind >> 4 {
        0xA => {
            plane.data.clear();
            plane.data.extend_from_slice(body);
        }
        0xB => snappy_into(body, &mut plane.data, true)?,
        0xC => decode_chunks(body, &mut plane.data)?,
        _ => return Err(HapError::UnsupportedFormat(kind)),
    }
    let expected = format.image_bytes(width, height);
    if plane.data.len() != expected {
        return Err(HapError::WrongSize {
            expected,
            actual: plane.data.len(),
        });
    }
    Ok(())
}

/// Decompresses Snappy data, replacing `out` or appending to it.
fn snappy_into(compressed: &[u8], out: &mut Vec<u8>, replace: bool) -> Result<(), HapError> {
    if replace {
        out.clear();
    }
    let start = out.len();
    let length = snap::raw::decompress_len(compressed)?;
    out.resize(start + length, 0);
    snap::raw::Decoder::new().decompress(compressed, &mut out[start..])?;
    Ok(())
}

fn le_u32_table(table: &[u8]) -> impl Iterator<Item = usize> + '_ {
    table
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes) as usize)
}

/// Decodes frame data split into independently compressed chunks.
fn decode_chunks(body: &[u8], out: &mut Vec<u8>) -> Result<(), HapError> {
    let (kind, container, frame_data) = section(body)?;
    if kind != INSTRUCTIONS_CONTAINER {
        return Err(HapError::BadInstructions(
            "missing decode instructions container",
        ));
    }
    let (mut compressors, mut sizes, mut offsets) = (None, None, None);
    let mut rest = container;
    while !rest.is_empty() {
        let (kind, table, after) = section(rest)?;
        match kind {
            COMPRESSOR_TABLE => compressors = Some(table),
            SIZE_TABLE => sizes = Some(table),
            OFFSET_TABLE => offsets = Some(table),
            // Unknown sections are skipped, as the specification asks.
            _ => {}
        }
        rest = after;
    }
    let compressors = compressors.ok_or(HapError::BadInstructions("missing compressor table"))?;
    let sizes = sizes.ok_or(HapError::BadInstructions("missing chunk size table"))?;
    let count = compressors.len();
    if sizes.len() != count * 4 || offsets.is_some_and(|table| table.len() != count * 4) {
        return Err(HapError::BadInstructions("tables of different lengths"));
    }

    out.clear();
    let mut next_offset = 0usize;
    let mut offset_table = offsets.map(le_u32_table);
    for (&compressor, size) in compressors.iter().zip(le_u32_table(sizes)) {
        let offset = match offset_table.as_mut() {
            Some(table) => table.next().unwrap_or(next_offset),
            None => next_offset,
        };
        let end = offset.checked_add(size).ok_or(HapError::Truncated)?;
        let chunk = frame_data.get(offset..end).ok_or(HapError::Truncated)?;
        next_offset = end;
        match compressor {
            CHUNK_UNCOMPRESSED => out.extend_from_slice(chunk),
            CHUNK_SNAPPY => snappy_into(chunk, out, false)?,
            other => return Err(HapError::UnknownCompressor(other)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_long_section_headers() {
        let short = [3, 0, 0, 0xAB, 1, 2, 3, 9];
        let (kind, body, rest) = section(&short).unwrap();
        assert_eq!((kind, body, rest), (0xAB, &[1, 2, 3][..], &[9][..]));

        let long = [0, 0, 0, 0xBE, 2, 0, 0, 0, 7, 8];
        let (kind, body, rest) = section(&long).unwrap();
        assert_eq!((kind, body, rest), (0xBE, &[7, 8][..], &[][..]));
    }

    #[test]
    fn truncated_sections_are_rejected() {
        assert!(matches!(
            section(&[5, 0, 0, 0xAB, 1]),
            Err(HapError::Truncated)
        ));
        assert!(matches!(
            section(&[0, 0, 0, 0xAB, 1]),
            Err(HapError::Truncated)
        ));
        assert!(matches!(section(&[1, 0]), Err(HapError::Truncated)));
    }

    #[test]
    fn block_sizes() {
        assert_eq!(BlockFormat::Bc1.image_bytes(1920, 1080), 480 * 270 * 8);
        assert_eq!(BlockFormat::Bc3.image_bytes(30, 18), 8 * 5 * 16);
    }

    #[test]
    fn unknown_instruction_sections_are_skipped() {
        use crate::test_support::{solid_plane, write_section};

        let texture = solid_plane(BlockFormat::Bc1, 8, 4, [255, 0, 0, 255]);
        let (first, second) = texture.split_at(8);
        let mut tables = Vec::new();
        write_section(
            &mut tables,
            COMPRESSOR_TABLE,
            &[CHUNK_UNCOMPRESSED; 2],
            false,
        );
        write_section(&mut tables, 0x7F, &[1, 2, 3], false);
        let sizes: Vec<u8> = [8u32, 8].iter().flat_map(|s| s.to_le_bytes()).collect();
        write_section(&mut tables, SIZE_TABLE, &sizes, false);
        let mut body = Vec::new();
        write_section(&mut body, INSTRUCTIONS_CONTAINER, &tables, false);
        body.extend(first);
        body.extend(second);
        let mut data = Vec::new();
        write_section(&mut data, 0xCB, &body, false);

        let mut frame = HapFrame::default();
        decode_frame(&data, 8, 4, &mut frame).unwrap();
        assert_eq!(frame.color.data, texture);
    }

    #[test]
    fn hdr_formats_are_unsupported() {
        let frame = [0, 0, 0, 0xA2, 0, 0, 0, 0];
        let mut out = HapFrame::default();
        assert!(matches!(
            decode_frame(&frame, 4, 4, &mut out),
            Err(HapError::UnsupportedFormat(0xA2))
        ));
    }
}
