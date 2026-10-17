//! Syphon, round trip in this process: a server publishes an image, a
//! client of it reads it back. Needs a GPU; without one the test prints a
//! notice and passes.

#![cfg(target_os = "macos")]
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use prism_io_video::{SyphonClient, SyphonServer};
use prism_render::{Gpu, pixels_texture, wgpu};

/// The first pixel of an 8-bit BGRA texture, as red, green, blue, alpha.
fn first_pixel(gpu: &Gpu, texture: &wgpu::Texture) -> [u8; 4] {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let bytes = buffer.slice(..).get_mapped_range().unwrap();
    [bytes[2], bytes[1], bytes[0], bytes[3]]
}

#[test]
fn a_published_image_is_read_back_by_a_client() {
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            return;
        }
    };
    // An orange program image.
    let (width, height) = (32, 16);
    let program = pixels_texture(
        &gpu.device,
        &gpu.queue,
        width,
        height,
        &vec![[1.0, 0.5, 0.0, 1.0]; (width * height) as usize],
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let mut server = SyphonServer::new("Prism round trip");
    server.publish(&gpu, &program, (width, height));
    assert!(server.is_publishing());
    let mut client: SyphonClient = server.local_client(&gpu).unwrap();
    assert!(client.is_valid());

    // Frames arrive once the copy is done on the GPU.
    let limit = Instant::now() + Duration::from_secs(5);
    loop {
        server.publish(&gpu, &program, (width, height));
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        if client.update(&gpu) {
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            let pixel = first_pixel(&gpu, client.texture().unwrap());
            if pixel[3] > 0 {
                assert_eq!(client.texture().unwrap().width(), width);
                assert!(
                    pixel[0] > 250 && (120..136).contains(&pixel[1]) && pixel[2] < 5,
                    "{pixel:?}"
                );
                break;
            }
        }
        assert!(Instant::now() < limit, "no frame received");
        std::thread::sleep(Duration::from_millis(20));
    }
}
