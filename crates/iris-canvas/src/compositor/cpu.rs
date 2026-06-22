// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! CPU composite path: inverse-maps every destination physical pixel back to
//! document space, so output coverage is complete at any DPI scale, zoom, or
//! rotation.
//!
//! The previous implementation forward-mapped each document pixel to a single
//! physical pixel; at DPI scale > 1 (or zoom > 1) that painted only one pixel
//! of each cluster and left the rest as background — visible as a dithered
//! checkerboard on high-DPI displays.

use std::sync::OnceLock;

use iris_pixel::{LayerContent, LayerTree, TileCoord, TileData, TILE_SIZE};

use crate::viewport::CanvasViewport;

/// Affine map from physical-pixel centres to document space.
///
/// `doc(sx, sy) = origin + sx·col_x + sy·col_y` — exact for the viewport's
/// similarity transform, derived numerically from [`CanvasViewport::screen_to_doc`]
/// so it can never drift from the event-handler mapping.
pub(crate) struct FrameMap {
    origin: kurbo::Vec2,
    col_x: kurbo::Vec2,
    col_y: kurbo::Vec2,
}

impl FrameMap {
    pub(crate) fn new(
        viewport: &CanvasViewport,
        logical_w: u32,
        logical_h: u32,
        scale_x: f64,
        scale_y: f64,
    ) -> Self {
        let inv = |sx: f64, sy: f64| {
            viewport.screen_to_doc(
                kurbo::Vec2::new((sx + 0.5) / scale_x, (sy + 0.5) / scale_y),
                logical_w,
                logical_h,
            )
        };
        let origin = inv(0.0, 0.0);
        Self {
            origin,
            col_x: inv(1.0, 0.0) - origin,
            col_y: inv(0.0, 1.0) - origin,
        }
    }

    /// Document coordinate under the centre of physical pixel `(sx, sy)`.
    pub(crate) fn doc_at(&self, sx: f64, sy: f64) -> kurbo::Vec2 {
        self.origin + self.col_x * sx + self.col_y * sy
    }
}

/// Composite `tree` into a straight-alpha sRGB RGBA8 buffer of
/// `width_px × height_px` physical pixels. Pure CPU — unit-testable.
pub(crate) fn composite_rgba8(
    tree: &LayerTree,
    viewport: &CanvasViewport,
    width_px: u32,
    height_px: u32,
    scale: f64,
) -> Vec<u8> {
    let logical_w = ((width_px as f64) / scale.max(1.0)).round().max(1.0) as u32;
    let logical_h = ((height_px as f64) / scale.max(1.0)).round().max(1.0) as u32;
    let scale_x = width_px as f64 / logical_w as f64;
    let scale_y = height_px as f64 / logical_h as f64;
    let map = FrameMap::new(viewport, logical_w, logical_h, scale_x, scale_y);

    let pixel_count = width_px as usize * height_px as usize;
    // Premultiplied linear-light f32 RGBA accumulation buffer.
    let mut acc = vec![0.0_f32; pixel_count * 4];

    fill_document_background(
        &mut acc, viewport, &map,
        width_px, height_px, logical_w, logical_h, scale_x, scale_y,
        tree.canvas_width, tree.canvas_height,
    );

    let visible_rect = viewport.visible_doc_rect(logical_w, logical_h);
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
                if let Some(tile_data) = px.tiles.get(TileCoord { tx, ty }) {
                    blit_tile(
                        &mut acc, tile_data, tx, ty, offset_x, offset_y,
                        viewport, &map,
                        width_px, height_px, logical_w, logical_h, scale_x, scale_y,
                        layer.opacity,
                    );
                }
                // Absent tile = transparent = no contribution to composite.
            }
        }
    }

    // Convert premultiplied linear → straight-alpha sRGB u8.
    let lut = srgb_lut();
    let mut u8_data = vec![0u8; pixel_count * 4];
    for (src, dst) in acc.chunks_exact(4).zip(u8_data.chunks_exact_mut(4)) {
        let a = src[3].clamp(0.0, 1.0);
        if a > f32::EPSILON {
            let inv_a = 1.0 / a;
            dst[0] = lut[lut_index(src[0] * inv_a)];
            dst[1] = lut[lut_index(src[1] * inv_a)];
            dst[2] = lut[lut_index(src[2] * inv_a)];
            dst[3] = (a * 255.0 + 0.5) as u8;
        }
        // a == 0 → leave (0, 0, 0, 0).
    }
    u8_data
}

/// Physical-pixel AABB covering a document-space rect (handles rotation by
/// projecting all four corners). Returns `(x0, y0, x1, y1)` clamped to the
/// frame, with `x1`/`y1` exclusive.
fn doc_rect_physical_aabb(
    viewport: &CanvasViewport,
    doc_x0: f64, doc_y0: f64, doc_x1: f64, doc_y1: f64,
    logical_w: u32, logical_h: u32,
    scale_x: f64, scale_y: f64,
    physical_w: u32, physical_h: u32,
) -> (usize, usize, usize, usize) {
    let corners = [
        kurbo::Vec2::new(doc_x0, doc_y0),
        kurbo::Vec2::new(doc_x1, doc_y0),
        kurbo::Vec2::new(doc_x0, doc_y1),
        kurbo::Vec2::new(doc_x1, doc_y1),
    ];
    let (mut x0, mut y0) = (f64::INFINITY, f64::INFINITY);
    let (mut x1, mut y1) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in corners {
        let s = viewport.doc_to_screen(c, logical_w, logical_h);
        x0 = x0.min(s.x * scale_x);
        y0 = y0.min(s.y * scale_y);
        x1 = x1.max(s.x * scale_x);
        y1 = y1.max(s.y * scale_y);
    }
    (
        (x0.floor().max(0.0)) as usize,
        (y0.floor().max(0.0)) as usize,
        (x1.ceil().clamp(0.0, physical_w as f64)) as usize,
        (y1.ceil().clamp(0.0, physical_h as f64)) as usize,
    )
}

/// Inverse-map blit: walk the tile's physical-pixel footprint, sample the tile
/// pixel under each destination pixel centre (nearest), Porter-Duff over.
#[allow(clippy::too_many_arguments)] // internal hot path; a struct would obscure the maths
fn blit_tile(
    acc: &mut [f32],
    tile_data: &TileData,
    tx: u32, ty: u32,
    offset_x: f64, offset_y: f64,
    viewport: &CanvasViewport,
    map: &FrameMap,
    physical_w: u32, physical_h: u32,
    logical_w: u32, logical_h: u32,
    scale_x: f64, scale_y: f64,
    opacity: f32,
) {
    let ts = TILE_SIZE as usize;
    let tile_x0 = offset_x + tx as f64 * ts as f64;
    let tile_y0 = offset_y + ty as f64 * ts as f64;
    let (px0, py0, px1, py1) = doc_rect_physical_aabb(
        viewport,
        tile_x0, tile_y0, tile_x0 + ts as f64, tile_y0 + ts as f64,
        logical_w, logical_h, scale_x, scale_y, physical_w, physical_h,
    );

    let bytes = tile_data.bytes();
    for sy in py0..py1 {
        let mut doc = map.doc_at(px0 as f64, sy as f64);
        for sx in px0..px1 {
            let lx = (doc.x - tile_x0).floor();
            let ly = (doc.y - tile_y0).floor();
            doc += map.col_x;
            if lx < 0.0 || ly < 0.0 || lx >= ts as f64 || ly >= ts as f64 {
                continue; // rotation/rounding slack outside this tile
            }
            let ib = (ly as usize * ts + lx as usize) * 8; // 4 ch × 2 bytes (f16)
            let sa = f16_to_f32(u16::from_le_bytes([bytes[ib + 6], bytes[ib + 7]])) * opacity;
            if sa <= 0.0 {
                continue;
            }
            let sr = f16_to_f32(u16::from_le_bytes([bytes[ib],     bytes[ib + 1]]));
            let sg = f16_to_f32(u16::from_le_bytes([bytes[ib + 2], bytes[ib + 3]]));
            let sb = f16_to_f32(u16::from_le_bytes([bytes[ib + 4], bytes[ib + 5]]));
            let ob = (sy * physical_w as usize + sx) * 4;
            let inv = 1.0 - sa;
            // Porter-Duff "over": dst = src_premul + dst × (1 − src_alpha)
            acc[ob    ] = sr * sa + acc[ob    ] * inv;
            acc[ob + 1] = sg * sa + acc[ob + 1] * inv;
            acc[ob + 2] = sb * sa + acc[ob + 2] * inv;
            acc[ob + 3] =    sa   + acc[ob + 3] * inv;
        }
    }
}

/// Fill the physical pixels whose centres fall inside the document bounds with
/// opaque white. Inverse-mapped, so it is exact under rotation (the previous
/// AABB fill painted white outside a rotated document).
#[allow(clippy::too_many_arguments)] // internal hot path; mirrors blit_tile
fn fill_document_background(
    acc: &mut [f32],
    viewport: &CanvasViewport,
    map: &FrameMap,
    physical_w: u32, physical_h: u32,
    logical_w: u32, logical_h: u32,
    scale_x: f64, scale_y: f64,
    doc_w: u32,
    doc_h: u32,
) {
    let (px0, py0, px1, py1) = doc_rect_physical_aabb(
        viewport,
        0.0, 0.0, doc_w as f64, doc_h as f64,
        logical_w, logical_h, scale_x, scale_y, physical_w, physical_h,
    );
    let (dw, dh) = (doc_w as f64, doc_h as f64);
    for sy in py0..py1 {
        let mut doc = map.doc_at(px0 as f64, sy as f64);
        for sx in px0..px1 {
            let inside = doc.x >= 0.0 && doc.x < dw && doc.y >= 0.0 && doc.y < dh;
            doc += map.col_x;
            if inside {
                let ob = (sy * physical_w as usize + sx) * 4;
                // Premultiplied opaque white: (1, 1, 1, 1).
                acc[ob..ob + 4].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]);
            }
        }
    }
}

/// Convert IEEE 754 half-precision bits to f32.
/// Subnormal f16 values (exp=0, mant≠0) are treated as zero — adequate for pixels.
pub(crate) fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp  = ((bits as u32) & 0x7C00) >> 10;
    let mant = (bits as u32) & 0x03FF;
    f32::from_bits(match exp {
        0  => sign,                                    // ±zero or subnormal → zero
        31 => sign | 0x7F80_0000 | (mant << 13),       // ±inf / NaN pass-through
        e  => sign | ((e + 112) << 23) | (mant << 13), // normal: rebias 15→127
    })
}

const LUT_SIZE: usize = 4096;

fn lut_index(linear: f32) -> usize {
    (linear.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32) as usize
}

/// Linear-light → sRGB u8 lookup table. `powf` per channel per pixel was the
/// dominant cost of the conversion loop (~10M `powf` per frame at 1080p).
fn srgb_lut() -> &'static [u8; LUT_SIZE] {
    static LUT: OnceLock<[u8; LUT_SIZE]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut lut = [0u8; LUT_SIZE];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = to_srgb_u8(i as f32 / (LUT_SIZE - 1) as f32);
        }
        lut
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
