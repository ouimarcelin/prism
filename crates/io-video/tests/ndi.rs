//! NDI, round trip on this computer: a sender publishes an image, a
//! receiver of its source reads it back. Needs the NDI library and a GPU;
//! without them the test prints a notice and passes.

#![cfg(feature = "ndi")]
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use prism_io_video::{NdiReceiver, NdiSender, ndi_sources, ndi_version};
use prism_render::{Gpu, pixels_texture, wgpu};

#[test]
fn a_published_image_is_received_from_the_network() {
    if let Err(error) = ndi_version() {
        eprintln!("skipping NDI test: {error}");
        return;
    }
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            return;
        }
    };
    // An orange program image.
    let (width, height) = (64, 32);
    let program = pixels_texture(
        &gpu.device,
        &gpu.queue,
        width,
        height,
        &vec![[1.0, 0.5, 0.0, 1.0]; (width * height) as usize],
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let name = format!("Prism test {}", std::process::id());
    let mut sender = NdiSender::new(&name, 30.0);

    // The full name of the source is known once the sender runs.
    let limit = Instant::now() + Duration::from_secs(10);
    let source = loop {
        sender.publish(&gpu, &program, (width, height));
        assert_eq!(sender.error(), None);
        if let Some(source) = sender.source_name() {
            break source;
        }
        assert!(Instant::now() < limit, "source not published");
        std::thread::sleep(Duration::from_millis(30));
    };
    assert!(source.contains(&name), "{source}");

    let receiver = NdiReceiver::connect(&source).unwrap();
    let limit = Instant::now() + Duration::from_secs(15);
    let frame = loop {
        sender.publish(&gpu, &program, (width, height));
        if let Some(frame) = receiver.take_frame() {
            break frame;
        }
        assert!(Instant::now() < limit, "no frame received");
        std::thread::sleep(Duration::from_millis(30));
    };
    assert!(receiver.is_connected());
    assert_eq!((frame.width, frame.height), (width, height));
    // Blue, green, red, alpha; NDI compresses, so values are close only.
    let pixel = &frame.pixels[..4];
    assert!(
        pixel[0] < 20 && (110..150).contains(&pixel[1]) && pixel[2] > 235,
        "{pixel:?}"
    );
    assert!(sender.connections() >= 1);
    // The finder of the process sees the source too.
    let limit = Instant::now() + Duration::from_secs(10);
    while !ndi_sources().contains(&source) {
        assert!(Instant::now() < limit, "source not found");
        std::thread::sleep(Duration::from_millis(100));
    }
}
