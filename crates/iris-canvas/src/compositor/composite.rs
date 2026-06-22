// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Core composite logic: layer iteration, tile upload, blend pass invocation.

use iris_pixel::{BlendMode, LayerContent, LayerTree, TILE_SIZE, TileCoord};

use crate::viewport::CanvasViewport;

use super::pass::{BlendPass, TileEntry, TileParamsGpu};
use super::upload;
use super::CompositorError;

/// Composite all visible pixel layers in `tree` into a single `Rgba16Float` texture.
///
/// Called from [`Compositor::composite`]; receives the lazily-initialised blend pipeline.
pub(super) fn run(
    blend: &BlendPass,
    tree: &LayerTree,
    viewport: &CanvasViewport,
    width_px: u32,
    height_px: u32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<appthere_canvas::GpuTexture, CompositorError> {
    let visible_rect = viewport.visible_doc_rect(width_px, height_px);
    let mut tiles: Vec<TileEntry> = Vec::new();

    // Collect layers bottom-to-top (DFS is top-first → reverse).
    let layers: Vec<_> = tree.iter_depth_first().collect();
    for layer in layers.iter().rev() {
        if !layer.visible {
            continue;
        }
        let LayerContent::Pixel(ref px) = layer.content else {
            // TODO(iris): SPEC.md §6.2 — Phase 3+: vector/text layer compositing
            tracing::warn!(
                layer_id = ?layer.id,
                "iris-canvas Phase 2: non-pixel layer skipped in compositor"
            );
            continue;
        };
        if layer.blend_mode != BlendMode::Normal {
            // The CPU path (`cpu.rs`) composites all 27 modes via the reference
            // `iris_pixel::blend`. The GPU path still skips non-Normal until the
            // programmable-blend shader lands (WebGPU has no framebuffer fetch,
            // so this needs a ping-pong / compute pass that samples the backdrop).
            // TODO(iris): SPEC.md §4.8 — Phase 4: GPU blend shader matching iris_pixel::blend.
            tracing::warn!(
                layer_id = ?layer.id,
                mode = ?layer.blend_mode,
                "iris-canvas Phase 2: only Normal blend composited; skipping layer"
            );
            continue;
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
                let coord = TileCoord { tx, ty };
                let texture = match px.tiles.get(coord) {
                    Some(data) => upload::upload_tile(device, queue, data),
                    None => upload::transparent_tile(device, queue),
                };
                let view = texture.create_view(&Default::default());
                let params = tile_params(
                    viewport, offset_x, offset_y, tx, ty,
                    (width_px, height_px), layer.opacity,
                );
                tiles.push(TileEntry { view, params });
            }
        }
    }

    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iris-canvas composite-output"),
        size: wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let output_view = output.create_view(&Default::default());
    blend.run(device, queue, &tiles, &output_view)?;

    Ok(appthere_canvas::GpuTexture { inner: output, width: width_px, height: height_px })
}

/// Compute NDC quad corners for a tile at `(tx, ty)` given the current viewport.
pub(super) fn tile_params(
    vp: &CanvasViewport,
    offset_x: f64,
    offset_y: f64,
    tx: u32,
    ty: u32,
    screen_size: (u32, u32),
    opacity: f32,
) -> TileParamsGpu {
    let ts = TILE_SIZE as f64;
    let doc_x0 = offset_x + tx as f64 * ts;
    let doc_y0 = offset_y + ty as f64 * ts;
    quad_params(vp, doc_x0, doc_y0, doc_x0 + ts, doc_y0 + ts, screen_size, opacity)
}

/// Compute NDC quad corners for an arbitrary document-space rect.
pub(super) fn quad_params(
    vp: &CanvasViewport,
    doc_x0: f64,
    doc_y0: f64,
    doc_x1: f64,
    doc_y1: f64,
    screen_size: (u32, u32),
    opacity: f32,
) -> TileParamsGpu {
    let (width_px, height_px) = screen_size;

    let tl = vp.doc_to_screen(kurbo::Vec2::new(doc_x0, doc_y0), width_px, height_px);
    let tr = vp.doc_to_screen(kurbo::Vec2::new(doc_x1, doc_y0), width_px, height_px);
    let bl = vp.doc_to_screen(kurbo::Vec2::new(doc_x0, doc_y1), width_px, height_px);
    let br = vp.doc_to_screen(kurbo::Vec2::new(doc_x1, doc_y1), width_px, height_px);

    TileParamsGpu {
        corner_tl: screen_to_ndc(tl, width_px, height_px),
        corner_tr: screen_to_ndc(tr, width_px, height_px),
        corner_bl: screen_to_ndc(bl, width_px, height_px),
        corner_br: screen_to_ndc(br, width_px, height_px),
        opacity,
        _pad: [0.0; 3],
    }
}

/// Convert a screen-space point to WebGPU NDC (x: [-1,1], y: [+1,-1]).
fn screen_to_ndc(screen: kurbo::Vec2, width_px: u32, height_px: u32) -> [f32; 2] {
    let nx = (screen.x / width_px as f64 * 2.0 - 1.0) as f32;
    // NDC Y is +1 at top, -1 at bottom (opposite of screen Y).
    let ny = (1.0 - screen.y / height_px as f64 * 2.0) as f32;
    [nx, ny]
}
