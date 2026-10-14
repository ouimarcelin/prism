//! Draws the control window into a PNG image, without window: to check the
//! layout of the interface.
//!
//! ```text
//! cargo run -p prism-app --example ui-snapshot -- OUTPUT.png [WIDTH HEIGHT]
//! ```
//!
//! The window shows a sample project: clips in the grid, the first one
//! playing, effects on the clip, its layer and the composition.

use std::process::ExitCode;

use prism_core::{Clip, Composition, Effect, Slot};
use prism_engine::{ClipState, EngineStatus, LayerStatus};
use prism_render::{Gpu, wgpu};
use prism_ui::{ControlPanel, Document, FrameInfo, Screen};

/// Format of the image, as in the control window.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn sample() -> (Document, Slot, EngineStatus) {
    let mut composition = Composition::default();
    let layers: Vec<_> = composition
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect();
    let columns: Vec<_> = composition
        .columns()
        .iter()
        .map(|column| column.id())
        .collect();
    for (index, layer) in layers.iter().enumerate().take(3) {
        for column in columns.iter().take(3) {
            let slot = Slot::new(*layer, *column);
            let _ = composition.set_clip(
                slot,
                Some(Clip::from_file(format!("/media/boucle-{index}.mov"))),
            );
        }
    }
    let slot = Slot::new(layers[0], columns[0]);
    let _ = composition.trigger_clip(slot);
    if let Some(clip) = composition.clip_mut(slot) {
        let mut blur = Effect::new("blur");
        blur.set_param("radius", 12.0);
        clip.effects = vec![blur, Effect::new("kaleidoscope")];
    }
    if let Some(layer) = composition.layer_mut(slot.layer) {
        let mut color = Effect::new("color");
        color.set_param("saturation", 1.6);
        layer.effects = vec![color];
    }
    composition.effects = vec![Effect::new("mirror")];
    let status = EngineStatus {
        fps: 60.0,
        frame_time_ms: 2.1,
        bpm: 128.0,
        beat: 2.3,
        layers: vec![LayerStatus {
            layer: Some(slot.layer),
            slot: Some(slot),
            state: ClipState::Playing,
            position: 1.5,
            duration: 4.0,
            frame: Some(90),
            frame_count: 240,
            ..LayerStatus::default()
        }],
        ..EngineStatus::default()
    };
    (Document::new(composition), slot, status)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: ui-snapshot OUTPUT.png [WIDTH HEIGHT]");
        return ExitCode::FAILURE;
    };
    let size = |index: usize, default: u32| {
        args.get(index)
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let (width, height) = (size(1, 1440), size(2, 900));
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let (document, slot, status) = sample();
    let mut panel = ControlPanel::new(document, Vec::new());
    panel.select(Some(slot));
    let screens = [Screen {
        name: "Intégré".into(),
        width: 2880,
        height: 1800,
        primary: true,
    }];
    let info = FrameInfo {
        status: &status,
        preview: None,
        screens: &screens,
        output_screen: None,
    };
    let ctx = egui::Context::default();
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32, height as f32),
        )),
        ..Default::default()
    };
    // A few frames, so sizes measured in one frame are used in the next.
    let mut renderer = egui_wgpu::Renderer::new(&gpu.device, FORMAT, Default::default());
    let mut output = None;
    for _ in 0..4 {
        let mut frame = ctx.run_ui(input(), |ui| {
            panel.ui(ui, &info);
        });
        for (id, deltas) in &frame.textures_delta.set {
            for delta in deltas {
                renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        frame.textures_delta.clear();
        output = Some(frame);
    }
    let Some(output) = output else {
        return ExitCode::FAILURE;
    };
    let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("snapshot"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [width, height],
        pixels_per_point: output.pixels_per_point,
    };
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let buffers = renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("snapshot"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
    }
    // Rows of a texture copy start every 256 bytes.
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot readback"),
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        target.size(),
    );
    gpu.queue
        .submit(buffers.into_iter().chain([encoder.finish()]));
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let pixels: Vec<u8> = match readback.slice(..).get_mapped_range() {
        Ok(mapped) => mapped
            .chunks(row as usize)
            .flat_map(|line| line[..width as usize * 4].to_vec())
            .collect(),
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let file = match std::fs::File::create(path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("error: cannot write {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let written = encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&pixels));
    if let Err(error) = written {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    println!("image     {path}");
    ExitCode::SUCCESS
}
