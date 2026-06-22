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

use iris_pixel::{blend, BlendMode, LayerContent, LayerTree, TileCoord, TileData, TILE_SIZE};

use crate::viewport::CanvasViewport;

use super::cpu_color::{f16_to_f32, lut_index, srgb_lut};

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
                        layer.opacity, layer.blend_mode,
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
    mode: BlendMode,
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
            let mut sr = f16_to_f32(u16::from_le_bytes([bytes[ib],     bytes[ib + 1]]));
            let mut sg = f16_to_f32(u16::from_le_bytes([bytes[ib + 2], bytes[ib + 3]]));
            let mut sb = f16_to_f32(u16::from_le_bytes([bytes[ib + 4], bytes[ib + 5]]));
            let ob = (sy * physical_w as usize + sx) * 4;
            let inv = 1.0 - sa;
            let ad = acc[ob + 3];
            // For non-Normal modes, replace the source colour with the blended
            // colour mixed by backdrop coverage (W3C §blending):
            //   Cs' = (1 − αb)·Cs + αb·B(Cb, Cs)
            // then the same premultiplied source-over as Normal. The backdrop
            // straight colour Cb is the un-premultiplied accumulator.
            if mode != BlendMode::Normal && ad > f32::EPSILON {
                let inv_ad = 1.0 / ad;
                let cb = [acc[ob] * inv_ad, acc[ob + 1] * inv_ad, acc[ob + 2] * inv_ad];
                let b = blend(mode, cb, [sr, sg, sb]);
                sr = (1.0 - ad) * sr + ad * b[0];
                sg = (1.0 - ad) * sg + ad * b[1];
                sb = (1.0 - ad) * sb + ad * b[2];
            }
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

