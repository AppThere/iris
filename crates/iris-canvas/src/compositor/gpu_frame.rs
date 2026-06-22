// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! GPU frame compositor for the Dioxus paint bridge (Phase 4).
//!
//! Composites visible tiles on the GPU (reusing [`BlendPass`]) into a
//! premultiplied linear `Rgba16Float` target, then converts to straight-alpha
//! sRGB `Rgba8Unorm` in a present pass — the format `vello`'s
//! `register_texture` atlas copy expects (identical to the CPU path output).
//!
//! Tile pixel uploads are cached across frames keyed by
//! [`TileData::revision`], so an unchanged tile costs zero upload bandwidth
//! regardless of how many frames it stays on screen.

use std::collections::BTreeMap;

use iris_pixel::{BlendMode, LayerContent, LayerTree, TileCoord, TILE_SIZE};

use crate::viewport::CanvasViewport;

use super::composite::{quad_params, tile_params};
use super::pass::{BlendPass, TileEntry, TileParamsGpu};
use super::present::PresentPass;
use super::upload;
use super::CompositorError;

/// Tile textures are dropped after this many frames without use.
const TILE_EVICT_AFTER_FRAMES: u64 = 600;

struct CachedTile {
    view: wgpu::TextureView,
    last_used: u64,
}

/// Per-device GPU compositing state for the paint-bridge frame path.
pub(crate) struct GpuFrame {
    blend: BlendPass,
    present: PresentPass,
    /// Uploaded tile textures keyed by [`iris_pixel::TileData::revision`].
    tiles: BTreeMap<u64, CachedTile>,
    /// 1×1 opaque-white tile used to draw the document background quad.
    white: wgpu::TextureView,
    frame: u64,
}

impl GpuFrame {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            blend: BlendPass::new(device),
            present: PresentPass::new(device),
            tiles: BTreeMap::new(),
            white: white_pixel_view(device, queue),
            frame: 0,
        }
    }

    /// Composite `tree` and return a straight-alpha sRGB `Rgba8Unorm` texture
    /// of `width_px × height_px` physical pixels.
    pub(crate) fn composite(
        &mut self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        scale: f64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, CompositorError> {
        self.frame += 1;
        // Viewport transforms use logical (CSS) dimensions so they match
        // screen_to_doc() in the event handler; NDC quad corners are
        // resolution-independent, so rasterising at physical size applies the
        // DPI scale for free.
        let logical_w = ((width_px as f64) / scale.max(1.0)).round().max(1.0) as u32;
        let logical_h = ((height_px as f64) / scale.max(1.0)).round().max(1.0) as u32;
        let screen = (logical_w, logical_h);

        let mut entries: Vec<TileEntry> = Vec::new();

        // Document background: opaque white quad over the document rect.
        entries.push(TileEntry {
            view: self.white.clone(),
            params: doc_rect_params(viewport, tree, screen),
        });

        let visible_rect = viewport.visible_doc_rect(logical_w, logical_h);
        let layers: Vec<_> = tree.iter_depth_first().collect();
        for layer in layers.iter().rev() {
            if !layer.visible {
                continue;
            }
            let LayerContent::Pixel(ref px) = layer.content else {
                continue;
            };
            if layer.blend_mode != BlendMode::Normal {
                // No backdrop-sampling blend shader on the GPU path yet, so
                // non-Normal layers composite *as Normal* here — still visible,
                // rather than vanishing. The CPU reference path (cpu.rs) does the
                // real blend via iris_pixel::blend; the panel writes the mode and
                // it round-trips through AIF/PSD/ORA regardless.
                // TODO(iris): SPEC.md §4.8 — Phase 4: GPU blend shader matching iris_pixel::blend.
                tracing::trace!(layer_id = ?layer.id, mode = ?layer.blend_mode,
                    "GPU frame compositor: blending as Normal until blend shader lands");
            }
            let offset_x = px.canvas_offset_x as f64;
            let offset_y = px.canvas_offset_y as f64;
            let ts = TILE_SIZE as f64;
            let tx_min = ((visible_rect.x0 - offset_x) / ts).floor().max(0.0) as u32;
            let ty_min = ((visible_rect.y0 - offset_y) / ts).floor().max(0.0) as u32;
            let tx_max = ((visible_rect.x1 - offset_x) / ts).ceil().max(0.0) as u32;
            let ty_max = ((visible_rect.y1 - offset_y) / ts).ceil().max(0.0) as u32;
            for ty in ty_min..=ty_max {
                for tx in tx_min..=tx_max {
                    // Absent tile = transparent = no draw needed.
                    let Some(data) = px.tiles.get(TileCoord { tx, ty }) else {
                        continue;
                    };
                    let frame = self.frame;
                    let cached = self.tiles.entry(data.revision()).or_insert_with(|| {
                        let texture = upload::upload_tile(device, queue, data);
                        CachedTile {
                            view: texture.create_view(&Default::default()),
                            last_used: frame,
                        }
                    });
                    cached.last_used = frame;
                    entries.push(TileEntry {
                        view: cached.view.clone(),
                        params: tile_params(
                            viewport, offset_x, offset_y, tx, ty, screen, layer.opacity,
                        ),
                    });
                }
            }
        }

        let frame = self.frame;
        self.tiles
            .retain(|_, t| frame.saturating_sub(t.last_used) < TILE_EVICT_AFTER_FRAMES);

        // Linear premultiplied composite at physical resolution.
        let linear = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iris-canvas frame-linear"),
            size: wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let linear_view = linear.create_view(&Default::default());
        self.blend.run(device, queue, &entries, &linear_view)?;

        // COMPAT(blitz): Rgba8Unorm with STORAGE | TEXTURE_BINDING | COPY_SRC |
        // COPY_DST is what vello::Renderer::register_texture's atlas copy
        // accepts — identical to the CPU path's output texture.
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iris-canvas frame-srgb"),
            size: wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let output_view = output.create_view(&Default::default());
        self.present.run(device, queue, &linear_view, &output_view);

        Ok(output)
    }
}

/// NDC quad covering the document rect (the white background).
fn doc_rect_params(
    viewport: &CanvasViewport,
    tree: &LayerTree,
    screen: (u32, u32),
) -> TileParamsGpu {
    quad_params(
        viewport,
        0.0,
        0.0,
        tree.canvas_width as f64,
        tree.canvas_height as f64,
        screen,
        1.0,
    )
}

/// Build a 1×1 opaque-white `Rgba16Float` texture view (f16 1.0 = 0x3C00).
fn white_pixel_view(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iris-canvas white"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let one = 0x3C00_u16.to_le_bytes();
    let px = [one[0], one[1], one[0], one[1], one[0], one[1], one[0], one[1]];
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &px,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(8), rows_per_image: Some(1) },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    texture.create_view(&Default::default())
}
