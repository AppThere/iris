// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel compositor: composites all visible [`LayerTree`] layers into a single
//! `Rgba16Float` wgpu texture representing the current viewport.
//!
//! Phase 2: Normal blend mode only. All other blend modes log a warning and
//! the layer is skipped. See TODO below for Phase 4.

pub(crate) mod pass;
pub(crate) mod upload;
mod composite;

use std::sync::{Arc, Mutex};

use iris_pixel::{LayerContent, LayerTree, TileCoord, TileData, TILE_SIZE};

use crate::viewport::CanvasViewport;

use pass::BlendPass;

/// Errors produced by the compositor.
#[derive(Debug, thiserror::Error)]
pub enum CompositorError {
    /// A wgpu-level operation failed.
    #[error("wgpu compositor error: {0}")]
    Wgpu(String),
}

/// GPU pixel compositor, lazily initialised on first [`Compositor::composite`] call.
///
/// The [`BlendPass`] pipeline is created once and reused across frames.
/// `Arc<Mutex<...>>` allows the compositor to be shared between the Dioxus
/// component (which owns it) and the [`PageSource`] impl (Q5 from audit).
pub struct Compositor {
    state: Arc<Mutex<Option<BlendPass>>>,
}

impl Compositor {
    /// Create a new compositor. Pipeline is initialised lazily.
    pub fn new() -> Self {
        Self { state: Arc::new(Mutex::new(None)) }
    }

    /// Composite all visible pixel layers in `tree` and return the output texture.
    ///
    /// `device` and `queue` arrive from `PageSource::render()`. The pipeline is
    /// initialised on the first call and reused thereafter.
    pub fn composite(
        &self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<appthere_canvas::GpuTexture, CompositorError> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let blend = guard.get_or_insert_with(|| BlendPass::new(device));
        composite::run(blend, tree, viewport, width_px, height_px, device, queue)
    }

    /// CPU composite path — safe to call from `CustomPaintSource::render()`.
    ///
    /// Uses `queue.write_texture()` rather than a `CommandEncoder`, so it cannot
    /// corrupt Vello's in-progress encoder. Returns an `Rgba8Unorm` texture
    /// populated without creating or submitting a `CommandEncoder`.
    ///
    /// Phase 2: composites visible pixel tiles in CPU RAM using Porter-Duff over,
    /// then converts linear f32 → sRGB u8 for upload.
    /// TODO(iris): Phase 4 — replace with GPU compute path via render_to_texture().
    pub fn composite_to_texture(
        &self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, CompositorError> {
        let pixel_count = (width_px * height_px) as usize;
        // Premultiplied linear-light f32 RGBA accumulation buffer.
        let mut acc = vec![0.0_f32; pixel_count * 4];

        let visible_rect = viewport.visible_doc_rect(width_px, height_px);
        let layers: Vec<_> = tree.iter_depth_first().collect();

        for layer in layers.iter().rev() {
            if !layer.visible {
                continue;
            }
            let LayerContent::Pixel(ref px) = layer.content else {
                continue;
            };
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
                    if let Some(tile_data) = px.tiles.get(coord) {
                        blit_tile(
                            &mut acc, tile_data, tx, ty,
                            offset_x, offset_y,
                            viewport, width_px, height_px, layer.opacity,
                        );
                    }
                    // Absent tile = transparent = no contribution to composite.
                }
            }
        }

        // Convert premultiplied linear → straight-alpha sRGB u8 for upload.
        let mut u8_data = Vec::with_capacity(pixel_count * 4);
        for i in 0..pixel_count {
            let base = i * 4;
            let a = acc[base + 3].clamp(0.0, 1.0);
            let (r, g, b) = if a > f32::EPSILON {
                (acc[base] / a, acc[base + 1] / a, acc[base + 2] / a)
            } else {
                (0.0, 0.0, 0.0)
            };
            u8_data.push(to_srgb_u8(r));
            u8_data.push(to_srgb_u8(g));
            u8_data.push(to_srgb_u8(b));
            u8_data.push((a * 255.0 + 0.5) as u8);
        }

        // COMPAT(blitz): Rgba8Unorm + COPY_SRC + COPY_DST is required by
        // vello::Renderer::register_texture(), which copies the texture into
        // Vello's image atlas via a wgpu copy operation (needs COPY_SRC).
        // COPY_DST is required by queue.write_texture() for the initial upload.
        // TEXTURE_BINDING and STORAGE_BINDING are required by anyrender_vello's blit pass
        // which samples the texture in a shader (vello::Renderer::register_texture path).
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iris-canvas-composite"),
            size: wgpu::Extent3d {
                width: width_px, height: height_px, depth_or_array_layers: 1,
            },
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
}

impl Default for Compositor {
    fn default() -> Self { Self::new() }
}

// ── CPU composite helpers ─────────────────────────────────────────────────────

/// Forward-map a tile's pixels onto the accumulation buffer using Porter-Duff over.
///
/// Straight-alpha f16 source pixels are premultiplied before compositing.
/// Phase 2 note: only called when a tile has data; blank documents have no
/// tile data so this path is not exercised until pixels are painted.
fn blit_tile(
    acc: &mut [f32],
    tile_data: &TileData,
    tx: u32, ty: u32,
    offset_x: f64, offset_y: f64,
    viewport: &CanvasViewport,
    width_px: u32, height_px: u32,
    opacity: f32,
) {
    let ts = TILE_SIZE as usize;
    let bytes = &tile_data.0;
    for py in 0..ts {
        for px_local in 0..ts {
            let doc = kurbo::Vec2::new(
                offset_x + (tx as f64 * ts as f64) + px_local as f64 + 0.5,
                offset_y + (ty as f64 * ts as f64) + py as f64 + 0.5,
            );
            let screen = viewport.doc_to_screen(doc, width_px, height_px);
            let sx = screen.x as i64;
            let sy = screen.y as i64;
            if sx < 0 || sy < 0 || sx >= width_px as i64 || sy >= height_px as i64 {
                continue;
            }
            let ib = (py * ts + px_local) * 8; // 4 channels × 2 bytes per f16
            let sr = f16_to_f32(u16::from_le_bytes([bytes[ib],     bytes[ib + 1]]));
            let sg = f16_to_f32(u16::from_le_bytes([bytes[ib + 2], bytes[ib + 3]]));
            let sb = f16_to_f32(u16::from_le_bytes([bytes[ib + 4], bytes[ib + 5]]));
            let sa = f16_to_f32(u16::from_le_bytes([bytes[ib + 6], bytes[ib + 7]])) * opacity;
            let ob = (sy as usize * width_px as usize + sx as usize) * 4;
            let inv = 1.0 - sa;
            // Porter-Duff "over": dst = src_premul + dst × (1 − src_alpha)
            acc[ob    ] = sr * sa + acc[ob    ] * inv;
            acc[ob + 1] = sg * sa + acc[ob + 1] * inv;
            acc[ob + 2] = sb * sa + acc[ob + 2] * inv;
            acc[ob + 3] =    sa   + acc[ob + 3] * inv;
        }
    }
}

/// Convert IEEE 754 half-precision bits to f32.
/// Subnormal f16 values (exp=0, mant≠0) are treated as zero — adequate for pixels.
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp  = ((bits as u32) & 0x7C00) >> 10;
    let mant = (bits as u32) & 0x03FF;
    f32::from_bits(match exp {
        0  => sign,                                     // ±zero or subnormal → zero
        31 => sign | 0x7F80_0000 | (mant << 13),       // ±inf / NaN pass-through
        e  => sign | ((e + 112) << 23) | (mant << 13), // normal: rebias 15→127
    })
}

/// Encode a linear-light [0,1] value to sRGB gamma and clamp to u8.
fn to_srgb_u8(linear: f32) -> u8 {
    let s = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.clamp(0.0, 1.0).powf(1.0 / 2.4) - 0.055
    };
    (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}
