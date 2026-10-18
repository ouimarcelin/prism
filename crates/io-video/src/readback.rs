//! The program image read back on the CPU as 8-bit BGRA, a few frames late
//! and without waiting for the GPU: for outputs that take pixels (NDI®,
//! Spout).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use prism_render::{Gpu, wgpu};

use crate::convert::Converter;

/// An image read back: 8-bit BGRA rows, packed.
pub(crate) struct Picture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

/// A buffer the converted image is copied to, then read on the CPU.
struct Readback {
    buffer: wgpu::Buffer,
    size: (u32, u32),
    /// Set when the buffer is mapped and can be read.
    ready: Arc<AtomicBool>,
    pending: bool,
}

/// Buffers in flight: the GPU fills one while the CPU reads another.
const READBACKS: usize = 3;

/// Bytes per row of a copy, padded as the GPU requires.
fn padded_row(width: u32) -> u32 {
    (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}

/// Converts program images and reads them back, a few frames later.
#[derive(Default)]
pub(crate) struct BgraReader {
    converter: Converter,
    readbacks: Vec<Readback>,
    next: usize,
}

impl BgraReader {
    /// Converts `program` (16-bit float, `size` pixels) and starts reading
    /// it back. Dropped when every buffer is still in flight.
    pub(crate) fn submit(&mut self, gpu: &Gpu, program: &wgpu::TextureView, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        let Some(converted) = self.converter.convert(gpu, program, size) else {
            return;
        };
        if self.readbacks.len() < READBACKS {
            self.readbacks.push(Readback {
                buffer: readback_buffer(gpu, size),
                size,
                ready: Arc::default(),
                pending: false,
            });
        }
        let index = self.next % self.readbacks.len();
        let readback = &mut self.readbacks[index];
        if readback.pending {
            return;
        }
        self.next = self.next.wrapping_add(1);
        if readback.size != size {
            readback.buffer = readback_buffer(gpu, size);
            readback.size = size;
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            converted.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row(size.0)),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let ready = Arc::clone(&readback.ready);
        readback.pending = true;
        readback
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                ready.store(result.is_ok(), Ordering::Release);
            });
    }

    /// The images read back since the last call, oldest first.
    pub(crate) fn finished(&mut self, gpu: &Gpu) -> Vec<Picture> {
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let count = self.readbacks.len();
        let mut pictures = Vec::new();
        for offset in 0..count {
            // Oldest first: the buffer after the one filled last.
            let index = (self.next + offset) % count;
            let readback = &mut self.readbacks[index];
            if !readback.pending || !readback.ready.swap(false, Ordering::Acquire) {
                continue;
            }
            readback.pending = false;
            let (width, height) = readback.size;
            let row = padded_row(width) as usize;
            if let Ok(mapped) = readback.buffer.slice(..).get_mapped_range() {
                pictures.push(Picture {
                    width,
                    height,
                    pixels: mapped
                        .chunks(row)
                        .flat_map(|line| &line[..width as usize * 4])
                        .copied()
                        .collect(),
                });
            }
            readback.buffer.unmap();
        }
        pictures
    }
}

fn readback_buffer(gpu: &Gpu, size: (u32, u32)) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("program readback"),
        size: u64::from(padded_row(size.0) * size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_rows_are_padded() {
        assert_eq!(padded_row(1920), 7680);
        assert_eq!(padded_row(100), 512);
    }
}
