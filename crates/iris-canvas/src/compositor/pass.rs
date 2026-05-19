// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Render-pipeline compositor pass.
//!
//! [`BlendPass`] holds a cached wgpu render pipeline implementing Normal blend
//! via hardware `PREMULTIPLIED_ALPHA_BLENDING`. One pipeline, many draw calls.
//!
//! See `src/shaders/normal_blend.wgsl` for the vertex/fragment shaders.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt as _;

use super::CompositorError;

/// GPU-side representation of per-tile parameters.
/// Layout must match the WGSL `TileParams` struct (16-byte aligned, 48 bytes total).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct TileParamsGpu {
    /// NDC position of the top-left tile corner.
    pub corner_tl: [f32; 2],
    /// NDC position of the top-right tile corner.
    pub corner_tr: [f32; 2],
    /// NDC position of the bottom-left tile corner.
    pub corner_bl: [f32; 2],
    /// NDC position of the bottom-right tile corner.
    pub corner_br: [f32; 2],
    /// Layer opacity in [0.0, 1.0].
    pub opacity: f32,
    pub _pad: [f32; 3],
}

/// One tile entry: an uploaded texture view + its NDC quad corners + opacity.
pub(super) struct TileEntry {
    pub view: wgpu::TextureView,
    pub params: TileParamsGpu,
}

/// Cached wgpu render pipeline for Normal blend compositing.
pub(super) struct BlendPass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl BlendPass {
    const SHADER: &'static str =
        include_str!("../shaders/normal_blend.wgsl");

    /// Compile the blend shader and build all pipeline objects. Called once per device.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iris-canvas normal-blend shader"),
            source: wgpu::ShaderSource::Wgsl(Self::SHADER.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iris-canvas blend-bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("iris-canvas blend-pl"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iris-canvas blend-rp"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iris-canvas blend-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { pipeline, bind_group_layout, sampler }
    }

    /// Render all `tiles` (bottom-to-top order) into `output_view` with Normal blend.
    pub fn run(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tiles: &[TileEntry],
        output_view: &wgpu::TextureView,
    ) -> Result<(), CompositorError> {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("iris-canvas composite"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("iris-canvas composite-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);

            for tile in tiles {
                let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("iris-canvas tile-params"),
                    contents: bytemuck::bytes_of(&tile.params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("iris-canvas tile-bg"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&tile.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform_buf.as_entire_binding(),
                        },
                    ],
                });
                pass.set_bind_group(0, &bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
        Ok(())
    }
}
