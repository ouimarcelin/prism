//! Drawing of the program image into an output texture.

#![allow(clippy::unwrap_used)]

use prism_core::Resolution;
use prism_output::{Fit, Presenter};
use prism_render::{
    Compositor, Gpu, LayerInput, WORKING_FORMAT, read_texture, solid_color_texture, wgpu,
};

fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[test]
fn contain_letterboxes_and_clamps() {
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            return;
        }
    };
    // A 16 × 8 program, bright enough for additive overflow.
    let mut compositor = Compositor::new(
        &gpu.device,
        Resolution {
            width: 16,
            height: 8,
        },
    );
    let color = solid_color_texture(&gpu.device, &gpu.queue, [1.5, 0.25, 0.0, 1.0]);
    let view = color.create_view(&Default::default());
    let program = compositor.render(
        &gpu.device,
        &gpu.queue,
        &[LayerInput::new(&view, 1.0, prism_core::BlendMode::Normal)],
    );
    let program_view = program.create_view(&Default::default());

    // Into a 16 × 16 output: 4 black rows on top and bottom.
    let output = target(&gpu.device, 16, 16);
    let output_view = output.create_view(&Default::default());
    let presenter = Presenter::new(&gpu.device, WORKING_FORMAT);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    presenter.encode(
        &gpu.device,
        &mut encoder,
        &program_view,
        (16, 8),
        &output_view,
        (16, 16),
        Fit::Contain,
    );
    gpu.queue.submit([encoder.finish()]);
    let pixels = read_texture(&gpu.device, &gpu.queue, &output).unwrap();

    let at = |x: usize, y: usize| pixels[y * 16 + x];
    assert_eq!(at(8, 1), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(at(8, 14), [0.0, 0.0, 0.0, 1.0]);
    let inside = at(8, 8);
    assert!((inside[0] - 1.0).abs() < 1e-3, "clamped red {inside:?}");
    assert!((inside[1] - 0.25).abs() < 1e-3, "{inside:?}");
    assert_eq!(inside[3], 1.0);
}
