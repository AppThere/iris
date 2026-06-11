// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Upload wrapper for the CPU composite path: runs [`super::cpu::composite_rgba8`]
//! and writes the result into a fresh `Rgba8Unorm` texture via
//! `queue.write_texture` (no `CommandEncoder`, so Vello's in-progress encoder
//! is never corrupted).

use iris_pixel::LayerTree;

use crate::compositor::CompositorError;
use crate::viewport::CanvasViewport;

/// Composite and upload to a new `Rgba8Unorm` texture via `queue.write_texture`.
pub(crate) fn run(
    tree: &LayerTree,
    viewport: &CanvasViewport,
    width_px: u32,
    height_px: u32,
    scale: f64,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<wgpu::Texture, CompositorError> {
    let u8_data = super::cpu::composite_rgba8(tree, viewport, width_px, height_px, scale);

    // COMPAT(blitz): Rgba8Unorm + COPY_SRC + COPY_DST is required by
    // vello::Renderer::register_texture(), which copies the texture into
    // Vello's image atlas via a wgpu copy operation (needs COPY_SRC).
    // COPY_DST is required by queue.write_texture() for the initial upload.
    // TEXTURE_BINDING and STORAGE_BINDING are required by anyrender_vello's blit
    // pass which samples the texture in a shader (register_texture path).
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iris-canvas-composite"),
        size: wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &u8_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width_px * 4),
            rows_per_image: Some(height_px),
        },
        wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
    );
    Ok(texture)
}
