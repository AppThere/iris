// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Reverse of [`crate::layer_from_rgba8`]: read a pixel layer's tiles back out
//! as straight-alpha 8-bit sRGB pixels, for format adapters that export to
//! 8-bit containers (e.g. PSD).

use exr::prelude::f16;

use iris_pixel::{Layer, LayerContent, TileCoord, TILE_SIZE};

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
