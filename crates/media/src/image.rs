//! Still images (PNG, JPEG) and PNG sequences.

use std::cmp::Ordering;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

use crate::decoder::DecodeError;
use crate::source::{Frame, FrameSource, MediaInfo, RgbaImage};

/// File extensions of still images, lowercase.
pub const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// Decodes a PNG or JPEG file into RGBA pixels.
pub fn decode_image(path: &Path, image: &mut RgbaImage) -> Result<(), DecodeError> {
    let bytes = fs::read(path).map_err(DecodeError::Open)?;
    let is_png = bytes.starts_with(b"\x89PNG");
    let result = if is_png {
        decode_png(&bytes, image)
    } else {
        decode_jpeg(bytes, image)
    };
    result.map_err(|message| DecodeError::Image(format!("{} : {message}", path.display())))
}

fn decode_png(bytes: &[u8], image: &mut RgbaImage) -> Result<(), String> {
    let mut decoder = png::Decoder::new(BufReader::new(Cursor::new(bytes)));
    // Palettes and low bit depths become 8-bit channels.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let size = reader.output_buffer_size().ok_or("image trop grande")?;
    let mut buffer = vec![0; size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let pixels = (info.width as usize) * (info.height as usize);
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("palette non convertie".into()),
    };
    image.width = info.width;
    image.height = info.height;
    image.pixels.clear();
    image.pixels.reserve(pixels * 4);
    for row in buffer
        .chunks_exact(info.line_size)
        .take(info.height as usize)
    {
        for pixel in row[..info.width as usize * channels].chunks_exact(channels) {
            let rgba = match *pixel {
                [gray] => [gray, gray, gray, 255],
                [gray, alpha] => [gray, gray, gray, alpha],
                [r, g, b] => [r, g, b, 255],
                [r, g, b, a] => [r, g, b, a],
                _ => unreachable!("chunks of 1 to 4 bytes"),
            };
            image.pixels.extend_from_slice(&rgba);
        }
    }
    Ok(())
}

fn decode_jpeg(bytes: Vec<u8>, image: &mut RgbaImage) -> Result<(), String> {
    let options = zune_core::options::DecoderOptions::default()
        .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), options);
    let pixels = decoder.decode().map_err(|error| error.to_string())?;
    let info = decoder.info().ok_or("en-tête JPEG illisible")?;
    image.width = u32::from(info.width);
    image.height = u32::from(info.height);
    if pixels.len() != image.width as usize * image.height as usize * 4 {
        return Err("taille de pixels inattendue".into());
    }
    image.pixels = pixels;
    Ok(())
}

/// A single image, shown as a one-frame clip.
pub struct StillImage {
    info: MediaInfo,
    image: RgbaImage,
}

impl StillImage {
    /// Reads and decodes an image file.
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
        let mut image = RgbaImage::default();
        decode_image(path, &mut image)?;
        let format = path.extension().map_or_else(String::new, |ext| {
            ext.to_string_lossy().to_ascii_uppercase()
        });
        Ok(Self {
            info: MediaInfo::uniform(format, image.width, image.height, 1, 1.0),
            image,
        })
    }
}

impl FrameSource for StillImage {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn decode(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        if index != 0 {
            return Err(DecodeError::FrameOutOfRange { index, count: 1 });
        }
        let target = frame.rgba_mut();
        target.width = self.image.width;
        target.height = self.image.height;
        target.pixels.clone_from(&self.image.pixels);
        Ok(())
    }
}

/// PNG files of a folder, played in name order.
pub struct ImageSequence {
    info: MediaInfo,
    files: Vec<PathBuf>,
}

impl ImageSequence {
    /// Lists the PNG files of `folder`. All frames take the size of the
    /// first one.
    pub fn open(folder: &Path, rate: f64) -> Result<Self, DecodeError> {
        let mut files: Vec<PathBuf> = fs::read_dir(folder)
            .map_err(DecodeError::Open)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                    && !path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            })
            .collect();
        files.sort_by(|a, b| natural_order(&a.to_string_lossy(), &b.to_string_lossy()));
        let first = files.first().ok_or_else(|| {
            DecodeError::Image(format!("{} : aucune image PNG", folder.display()))
        })?;
        let mut image = RgbaImage::default();
        decode_image(first, &mut image)?;
        Ok(Self {
            info: MediaInfo::uniform("PNG", image.width, image.height, files.len(), rate),
            files,
        })
    }
}

impl FrameSource for ImageSequence {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn decode(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        let path = self.files.get(index).ok_or(DecodeError::FrameOutOfRange {
            index,
            count: self.files.len(),
        })?;
        let image = frame.rgba_mut();
        decode_image(path, image)?;
        if (image.width, image.height) != (self.info.width, self.info.height) {
            return Err(DecodeError::Image(format!(
                "{} : {} × {} au lieu de {} × {}",
                path.display(),
                image.width,
                image.height,
                self.info.width,
                self.info.height
            )));
        }
        Ok(())
    }
}

/// Compares names with their digit runs as numbers: `frame2` before
/// `frame10`.
pub fn natural_order(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        chars.next();
                    }
                    digits
                };
                let (x, y) = (take(&mut a), take(&mut b));
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = tx
                    .len()
                    .cmp(&ty.len())
                    .then_with(|| tx.cmp(ty))
                    .then_with(|| x.len().cmp(&y.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_sort_by_value() {
        let mut names = vec!["f10.png", "f2.png", "f1.png", "f02.png", "g.png", "f.png"];
        names.sort_by(|a, b| natural_order(a, b));
        assert_eq!(
            names,
            ["f.png", "f1.png", "f2.png", "f02.png", "f10.png", "g.png"]
        );
    }
}
