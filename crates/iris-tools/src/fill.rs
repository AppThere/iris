// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Flood fill tool: BFS fill of a contiguous region within tolerance.

use std::collections::{HashSet, VecDeque};

use iris_pixel::{Layer, LayerContent, LayerId, PixelLayer, TileCoord, TileData, TILE_SIZE};

/// Flood-fill a contiguous region of similar pixels on `layer` with `color`.
///
/// Starts at `start` in document space. Expands to neighbours whose colour is
/// within Euclidean RGB distance `tolerance` of the seed colour. Fills within
/// `doc_rect` only to prevent unbounded expansion on blank canvases.
///
/// Returns the set of tile coords that were modified.
pub fn flood_fill(
    start: kurbo::Vec2,
    color: [f32; 4],
    tolerance: f32,
    layer: &mut Layer,
    _layer_id: LayerId,
    doc_rect: kurbo::Rect,
) -> Vec<TileCoord> {
    let LayerContent::Pixel(ref mut pl) = layer.content else { return Vec::new(); };

    let sx = start.x.floor() as i64;
    let sy = start.y.floor() as i64;
    if sx < doc_rect.x0 as i64 || sx >= doc_rect.x1 as i64
        || sy < doc_rect.y0 as i64 || sy >= doc_rect.y1 as i64
    {
        return Vec::new();
    }

    let seed = read_pixel(pl, sx, sy);
    // No-op: filling with a color already within tolerance of seed.
    if color_dist(&seed, &color) <= tolerance { return Vec::new(); }

    let bx0 = doc_rect.x0.floor() as i64;
    let by0 = doc_rect.y0.floor() as i64;
    let bx1 = doc_rect.x1.ceil() as i64;
    let by1 = doc_rect.y1.ceil() as i64;

    let mut visited: HashSet<(i64, i64)> = HashSet::new();
    let mut queue: VecDeque<(i64, i64)> = VecDeque::new();
    let mut dirty: HashSet<TileCoord> = HashSet::new();

    visited.insert((sx, sy));
    queue.push_back((sx, sy));

    while let Some((px, py)) = queue.pop_front() {
        let current = read_pixel(pl, px, py);
        if color_dist(&seed, &current) > tolerance { continue; }
        write_pixel(pl, px, py, color, &mut dirty);
        for (nx, ny) in [(px-1,py),(px+1,py),(px,py-1),(px,py+1)] {
            if nx < bx0 || nx >= bx1 || ny < by0 || ny >= by1 { continue; }
            if visited.contains(&(nx, ny)) { continue; }
            visited.insert((nx, ny));
            if color_dist(&seed, &read_pixel(pl, nx, ny)) <= tolerance {
                queue.push_back((nx, ny));
            }
        }
    }

    dirty.into_iter().collect()
}

fn read_pixel(pl: &PixelLayer, x: i64, y: i64) -> [f32; 4] {
    let lx = x - pl.canvas_offset_x as i64;
    let ly = y - pl.canvas_offset_y as i64;
    if lx < 0 || ly < 0 { return [0.0; 4]; }
    let ts = TILE_SIZE as i64;
    let tx = (lx / ts) as u32;
    let ty = (ly / ts) as u32;
    let coord = TileCoord { tx, ty };
    let Some(tile) = pl.tiles.get(coord) else { return [0.0; 4]; };
    let px_x = (lx % ts) as usize;
    let px_y = (ly % ts) as usize;
    let bi = (px_y * TILE_SIZE as usize + px_x) * 8;
    if bi + 8 > tile.0.len() { return [0.0; 4]; }
    [
        f16_to_f32(u16::from_le_bytes([tile.0[bi],   tile.0[bi+1]])),
        f16_to_f32(u16::from_le_bytes([tile.0[bi+2], tile.0[bi+3]])),
        f16_to_f32(u16::from_le_bytes([tile.0[bi+4], tile.0[bi+5]])),
        f16_to_f32(u16::from_le_bytes([tile.0[bi+6], tile.0[bi+7]])),
    ]
}

fn write_pixel(pl: &mut PixelLayer, x: i64, y: i64, color: [f32; 4], dirty: &mut HashSet<TileCoord>) {
    let lx = x - pl.canvas_offset_x as i64;
    let ly = y - pl.canvas_offset_y as i64;
    if lx < 0 || ly < 0 { return; }
    let ts = TILE_SIZE as i64;
    let coord = TileCoord { tx: (lx / ts) as u32, ty: (ly / ts) as u32 };
    let px_x = (lx % ts) as usize;
    let px_y = (ly % ts) as usize;
    let mut tile = pl.tiles.get(coord).cloned()
        .unwrap_or_else(|| TileData::transparent(TILE_SIZE));
    let bi = (px_y * TILE_SIZE as usize + px_x) * 8;
    if bi + 8 > tile.0.len() { return; }
    // Porter-Duff over (premultiplied)
    let sa = color[3];
    let inv = 1.0 - sa;
    let dst_r = f16_to_f32(u16::from_le_bytes([tile.0[bi],   tile.0[bi+1]]));
    let dst_g = f16_to_f32(u16::from_le_bytes([tile.0[bi+2], tile.0[bi+3]]));
    let dst_b = f16_to_f32(u16::from_le_bytes([tile.0[bi+4], tile.0[bi+5]]));
    let dst_a = f16_to_f32(u16::from_le_bytes([tile.0[bi+6], tile.0[bi+7]]));
    tile.0[bi..bi+2].copy_from_slice(&f32_to_f16(color[0]*sa + dst_r*inv).to_le_bytes());
    tile.0[bi+2..bi+4].copy_from_slice(&f32_to_f16(color[1]*sa + dst_g*inv).to_le_bytes());
    tile.0[bi+4..bi+6].copy_from_slice(&f32_to_f16(color[2]*sa + dst_b*inv).to_le_bytes());
    tile.0[bi+6..bi+8].copy_from_slice(&f32_to_f16(sa + dst_a*inv).to_le_bytes());
    pl.tiles.insert(coord, tile);
    dirty.insert(coord);
}

/// Euclidean RGB distance (alpha ignored). Range: 0.0–sqrt(3) ≈ 1.73.
fn color_dist(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr*dr + dg*dg + db*db).sqrt()
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp  = ((bits as u32) & 0x7C00) >> 10;
    let mant = (bits as u32) & 0x03FF;
    f32::from_bits(match exp {
        0  => sign,
        31 => sign | 0x7F80_0000 | (mant << 13),
        e  => sign | ((e + 112) << 23) | (mant << 13),
    })
}

fn f32_to_f16(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp  = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    let mant = ((bits >> 13) & 0x03FF) as u16;
    if f.is_nan()  { return sign | 0x7E00; }
    if exp <= 0    { return sign; }
    if exp >= 31   { return sign | 0x7C00; }
    sign | ((exp as u16) << 10) | mant
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_pixel::{
        BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent,
        PixelLayer, TileCache, LINEAR_SRGB,
    };

    fn blank_layer() -> (Layer, LayerId) {
        let id = uuid::Uuid::new_v4();
        let layer = Layer {
            id, name: "L".into(), visible: true, locked: false,
            opacity: 1.0, blend_mode: BlendMode::Normal, clipping_mask: false, mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba, bit_depth: BitDepth::F16,
                color_space: LINEAR_SRGB, compression: ExrCompression::Zip,
                canvas_offset_x: 0, canvas_offset_y: 0, crop_bounds: None,
                tiles: TileCache::default(),
            }),
        };
        (layer, id)
    }

    fn alpha_at(layer: &Layer, x: i64, y: i64) -> f32 {
        let LayerContent::Pixel(ref pl) = layer.content else { return 0.0 };
        read_pixel(pl, x, y)[3]
    }

    #[test]
    fn fill_paints_transparent_region() {
        let (mut layer, id) = blank_layer();
        let doc = kurbo::Rect::new(0.0, 0.0, 100.0, 100.0);
        let dirty = flood_fill(
            kurbo::Vec2::new(10.0, 10.0), [1.0, 0.0, 0.0, 1.0], 0.0, &mut layer, id, doc,
        );
        assert!(!dirty.is_empty(), "fill should dirty tiles");
        assert!(alpha_at(&layer, 10, 10) > 0.9, "filled pixel is opaque");
        assert!(alpha_at(&layer, 200, 200) < 0.1, "out-of-bounds pixel untouched");
    }

    #[test]
    fn fill_same_color_is_noop() {
        let (mut layer, id) = blank_layer();
        let doc = kurbo::Rect::new(0.0, 0.0, 100.0, 100.0);
        // Seed is transparent [0,0,0,0], fill color within tolerance
        let dirty = flood_fill(
            kurbo::Vec2::new(10.0, 10.0), [0.0, 0.0, 0.0, 0.0], 0.0, &mut layer, id, doc,
        );
        assert!(dirty.is_empty(), "filling with same color is no-op");
    }
}
