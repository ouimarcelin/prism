//! Drawing of the program image into an output texture.

use prism_render::wgpu;

/// How the program image fills an output of another shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    /// Whole image visible, black bars on the sides or top and bottom.
    #[default]
    Contain,
    /// Image stretched to the whole output.
    Stretch,
}

/// Rectangle of the output covered by the image, in whole pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Where an image of `source` size lands in an output of `target` size.
pub fn fit_viewport(source: (u32, u32), target: (u32, u32), fit: Fit) -> Viewport {
    let (target_width, target_height) = (target.0 as f32, target.1 as f32);
    let full = Viewport {
        x: 0.0,
        y: 0.0,
        width: target_width,
        height: target_height,
    };
    if fit == Fit::Stretch || source.0 == 0 || source.1 == 0 {
        return full;
    }
    // Whole pixels, so the image edges stay sharp.
    let scale =
        (f64::from(target.0) / f64::from(source.0)).min(f64::from(target.1) / f64::from(source.1));
    let width = (f64::from(source.0) * scale)
        .round()
        .min(f64::from(target.0)) as f32;
    let height = (f64::from(source.1) * scale)
        .round()
        .min(f64::from(target.1)) as f32;
    Viewport {
        x: ((target_width - width) / 2.0).floor(),
        y: ((target_height - height) / 2.0).floor(),
        width,
        height,
    }
}

/// Pipeline drawing the program image into textures of one format.
pub struct Presenter {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Presenter {
    /// Creates a presenter for targets of `format`. Use a non-sRGB format:
    /// the program already holds display-space values.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("present.wgsl"));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("present"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    /// Records the drawing of `source` into `target`, black around the image.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        source_size: (u32, u32),
        target: &wgpu::TextureView,
        target_size: (u32, u32),
        fit: Fit,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let viewport = fit_viewport(source_size, target_size, fit);
        if viewport.width < 1.0 || viewport.height < 1.0 {
            return;
        }
        pass.set_viewport(
            viewport.x,
            viewport.y,
            viewport.width,
            viewport.height,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contain_adds_bars_on_the_short_side() {
        let wide = fit_viewport((1920, 1080), (1000, 1000), Fit::Contain);
        assert_eq!(
            wide,
            Viewport {
                x: 0.0,
                y: 218.0,
                width: 1000.0,
                height: 563.0
            }
        );
        let tall = fit_viewport((1000, 1000), (1920, 1080), Fit::Contain);
        assert_eq!(
            tall,
            Viewport {
                x: 420.0,
                y: 0.0,
                width: 1080.0,
                height: 1080.0
            }
        );
    }

    #[test]
    fn same_aspect_and_stretch_cover_everything() {
        let full = Viewport {
            x: 0.0,
            y: 0.0,
            width: 3840.0,
            height: 2160.0,
        };
        assert_eq!(fit_viewport((1920, 1080), (3840, 2160), Fit::Contain), full);
        assert_eq!(fit_viewport((1000, 1000), (3840, 2160), Fit::Stretch), full);
        assert_eq!(fit_viewport((0, 0), (3840, 2160), Fit::Contain), full);
    }
}
