// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Reverse of [`crate::layer_from_rgba8`]: read a pixel layer's tiles back out
//! as straight-alpha 8-bit sRGB pixels, for format adapters that export to
//! 8-bit containers (e.g. PSD).

use std::io::Cursor;

use exr::prelude::f16;
use image::{ImageFormat, RgbaImage};

use iris_pixel::{Layer, LayerContent, LayerTree, TileCoord, TILE_SIZE};

use crate::error::AifError;

/// A pixel layer flattened to a contiguous 8-bit sRGB RGBA buffer.
#[derive(Debug, Clone)]
pub struct LayerPixels {
    /// Layer top-left X offset from the canvas origin.
    pub offset_x: i32,
    /// Layer top-left Y offset from the canvas origin.
    pub offset_y: i32,
    /// Buffer width in pixels.
    pub width: u32,
    /// Buffer height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, `[R, G, B, A, …]`, straight alpha, sRGB gamma.
    pub rgba: Vec<u8>,
}

/// Convert a linear-light value to sRGB gamma (inverse of `srgb_to_linear`).
pub fn linear_to_srgb(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Read a pixel [`Layer`]'s tiles into a straight-alpha 8-bit sRGB buffer sized
/// to the layer's crop bounds. Returns `None` for non-pixel layers or pixel
/// layers without crop bounds (extent unknown).
pub fn layer_to_rgba8(layer: &Layer) -> Option<LayerPixels> {
    let LayerContent::Pixel(ref px) = layer.content else {
        return None;
    };
    let bounds = px.crop_bounds.as_ref()?;
    let (w, h) = (bounds.width, bounds.height);

    let mut rgba = vec![0u8; w as usize * h as usize * 4];
    let cols = w.div_ceil(TILE_SIZE);
    let rows = h.div_ceil(TILE_SIZE);

    for ty in 0..rows {
        for tx in 0..cols {
            let Some(tile) = px.tiles.get(TileCoord { tx, ty }) else {
                continue; // absent tile = transparent
            };
            for ly in 0..TILE_SIZE {
                let gy = ty * TILE_SIZE + ly;
                if gy >= h {
                    break;
                }
                for lx in 0..TILE_SIZE {
                    let gx = tx * TILE_SIZE + lx;
                    if gx >= w {
                        break;
                    }
                    let ti = (ly as usize * TILE_SIZE as usize + lx as usize) * 8;
                    let chan = |o: usize| -> f32 {
                        f16::from_bits(u16::from_le_bytes([tile.0[ti + o], tile.0[ti + o + 1]]))
                            .to_f32()
                    };
                    let di = (gy as usize * w as usize + gx as usize) * 4;
                    rgba[di] = to_u8(linear_to_srgb(chan(0)));
                    rgba[di + 1] = to_u8(linear_to_srgb(chan(2)));
                    rgba[di + 2] = to_u8(linear_to_srgb(chan(4)));
                    rgba[di + 3] = to_u8(chan(6));
                }
            }
        }
    }

    Some(LayerPixels {
        offset_x: px.canvas_offset_x,
        offset_y: px.canvas_offset_y,
        width: w,
        height: h,
        rgba,
    })
}

/// Flatten all visible pixel layers in `tree` into a straight-alpha 8-bit sRGB
/// buffer of `width * height * 4` bytes, for adapters that need a merged image
/// (PSD's image-data section, ORA's `mergedimage.png`).
///
/// Compositing uses Porter-Duff "over" in sRGB space. Group-level
/// opacity/blend are not applied (group nodes carry no pixels); non-Normal
/// blend modes fall back to "over".
// TODO(iris): SPEC.md §6.2 — composite in linear light and honour group/blend
// once the shared compositor is reusable outside iris-canvas.
pub fn flatten_to_rgba8(tree: &LayerTree, width: u32, height: u32) -> Vec<u8> {
    let mut canvas = vec![0u8; width as usize * height as usize * 4];

    // iter_depth_first is top-first; composite bottom-to-top.
    let layers: Vec<_> = tree.iter_depth_first().collect();
    for layer in layers.iter().rev() {
        if !layer.visible {
            continue;
        }
        let Some(px) = layer_to_rgba8(layer) else {
            continue;
        };
        let layer_alpha = layer.opacity.clamp(0.0, 1.0);
        for y in 0..px.height {
            let cy = px.offset_y + y as i32;
            if cy < 0 || cy >= height as i32 {
                continue;
            }
            for x in 0..px.width {
                let cx = px.offset_x + x as i32;
                if cx < 0 || cx >= width as i32 {
                    continue;
                }
                let si = ((y * px.width + x) * 4) as usize;
                let di = ((cy as u32 * width + cx as u32) * 4) as usize;
                over(&px.rgba[si..si + 4], &mut canvas[di..di + 4], layer_alpha);
            }
        }
    }
    canvas
}

/// Porter-Duff "over": composite straight-alpha `src` (scaled by `opacity`)
/// onto straight-alpha `dst` in place.
fn over(src: &[u8], dst: &mut [u8], opacity: f32) {
    let sa = (src[3] as f32 / 255.0) * opacity;
    if sa <= 0.0 {
        return;
    }
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    for c in 0..3 {
        let s = src[c] as f32 / 255.0;
        let d = dst[c] as f32 / 255.0;
        let out = (s * sa + d * da * (1.0 - sa)) / out_a;
        dst[c] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}

/// Encode a straight-alpha 8-bit RGBA buffer (`width * height * 4` bytes) as PNG.
pub fn encode_png_rgba8(width: u32, height: u32, rgba8: &[u8]) -> Result<Vec<u8>, AifError> {
    let img = RgbaImage::from_raw(width, height, rgba8.to_vec())
        .ok_or_else(|| AifError::ImportError("PNG encode: RGBA buffer size mismatch".to_string()))?;
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| AifError::ImportError(format!("PNG encode failed: {e}")))?;
    Ok(out.into_inner())
}
