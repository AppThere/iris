// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Hot-loop painting primitives: stamp, paint_tile, interpolate, f16 codec.
//! Called exclusively from [`super::BrushEngine`].

use std::collections::HashSet;

use iris_pixel::{Layer, LayerContent, LayerId, PixelLayer, TileCoord, TileData, TILE_SIZE};

pub(super) fn stamp(
    last_pos: &mut Option<kurbo::Vec2>,
    dirty_tiles: &mut HashSet<(LayerId, TileCoord)>,
    settings: &super::BrushSettings,
    pos: kurbo::Vec2,
    pressure: f32,
    layer: &mut Layer,
    layer_id: LayerId,
) {
    let size = if settings.pressure_size { settings.size_px * pressure } else { settings.size_px };
    let radius = (size / 2.0).max(0.5) as f64;
    let pixel_layer = match &mut layer.content {
        LayerContent::Pixel(pl) => pl,
        _ => return,
    };
    let off_x = pixel_layer.canvas_offset_x as i64;
    let off_y = pixel_layer.canvas_offset_y as i64;
    let ts = TILE_SIZE as i64;
    let tx0 = ((pos.x - radius) as i64 - off_x).div_euclid(ts) as i32;
    let ty0 = ((pos.y - radius) as i64 - off_y).div_euclid(ts) as i32;
    let tx1 = ((pos.x + radius) as i64 + 1 - off_x).div_euclid(ts) as i32;
    let ty1 = ((pos.y + radius) as i64 + 1 - off_y).div_euclid(ts) as i32;
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            if tx < 0 || ty < 0 { continue; }
            let coord = TileCoord { tx: tx as u32, ty: ty as u32 };
            paint_tile(pos, radius, settings, coord, pixel_layer, layer_id, dirty_tiles);
        }
    }
    *last_pos = Some(pos);
}

pub(super) fn interpolate(
    last_pos: &mut Option<kurbo::Vec2>,
    dist_acc: &mut f64,
    dirty_tiles: &mut HashSet<(LayerId, TileCoord)>,
    settings: &super::BrushSettings,
    new_pos: kurbo::Vec2,
    pressure: f32,
    layer: &mut Layer,
    layer_id: LayerId,
) {
    let Some(last) = *last_pos else {
        stamp(last_pos, dirty_tiles, settings, new_pos, pressure, layer, layer_id);
        return;
    };
    let spacing = (settings.size_px as f64 * settings.spacing as f64).max(1.0);
    let delta = new_pos - last;
    let seg_len = delta.hypot();
    if seg_len < 1e-6 { return; }
    let mut covered = spacing - *dist_acc;
    while covered <= seg_len {
        let t = covered / seg_len;
        stamp(last_pos, dirty_tiles, settings, last + delta * t, pressure, layer, layer_id);
        covered += spacing;
    }
    *dist_acc = seg_len - (covered - spacing);
    *last_pos = Some(new_pos);
}

fn paint_tile(
    center: kurbo::Vec2,
    radius: f64,
    settings: &super::BrushSettings,
    coord: TileCoord,
    pl: &mut PixelLayer,
    layer_id: LayerId,
    dirty_tiles: &mut HashSet<(LayerId, TileCoord)>,
) {
    let mut tile = pl.tiles.get(coord).cloned()
        .unwrap_or_else(|| TileData::transparent(TILE_SIZE));
    let tile_ox = pl.canvas_offset_x as f64 + coord.tx as f64 * TILE_SIZE as f64;
    let tile_oy = pl.canvas_offset_y as f64 + coord.ty as f64 * TILE_SIZE as f64;
    let color = settings.color;
    let opacity = settings.opacity;
    let hardness = settings.hardness;
    let erase = settings.erase_mode;
    let selection = settings.selection;
    let ts = TILE_SIZE as usize;
    {
        let bytes = &mut tile.0;
        for py in 0..ts {
            for px in 0..ts {
                let wx = tile_ox + px as f64 + 0.5;
                let wy = tile_oy + py as f64 + 0.5;
                if let Some(sel) = selection {
                    if !sel.contains(kurbo::Point::new(wx, wy)) { continue; }
                }
                let dist = ((wx - center.x).powi(2) + (wy - center.y).powi(2)).sqrt();
                if dist > radius { continue; }
                let falloff_start = radius * hardness as f64;
                let alpha = if hardness >= 1.0 || dist <= falloff_start {
                    opacity
                } else {
                    let t = ((dist - falloff_start) / (radius - falloff_start)) as f32;
                    opacity * (1.0 - t)
                };
                let bi = (py * ts + px) * 8;
                let dst_r = f16_to_f32(u16::from_le_bytes([bytes[bi],   bytes[bi+1]]));
                let dst_g = f16_to_f32(u16::from_le_bytes([bytes[bi+2], bytes[bi+3]]));
                let dst_b = f16_to_f32(u16::from_le_bytes([bytes[bi+4], bytes[bi+5]]));
                let dst_a = f16_to_f32(u16::from_le_bytes([bytes[bi+6], bytes[bi+7]]));
                let (out_r, out_g, out_b, out_a) = if erase {
                    let f = 1.0 - alpha;
                    (dst_r * f, dst_g * f, dst_b * f, dst_a * f)
                } else {
                    let src_a = color[3] * alpha;
                    let f = 1.0 - src_a;
                    (color[0]*src_a + dst_r*f, color[1]*src_a + dst_g*f,
                     color[2]*src_a + dst_b*f, src_a + dst_a*f)
                };
                bytes[bi..bi+2].copy_from_slice(&f32_to_f16(out_r).to_le_bytes());
                bytes[bi+2..bi+4].copy_from_slice(&f32_to_f16(out_g).to_le_bytes());
                bytes[bi+4..bi+6].copy_from_slice(&f32_to_f16(out_b).to_le_bytes());
                bytes[bi+6..bi+8].copy_from_slice(&f32_to_f16(out_a).to_le_bytes());
            }
        }
    }
    pl.tiles.insert(coord, tile);
    dirty_tiles.insert((layer_id, coord));
}

pub(super) fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp  = ((bits as u32) & 0x7C00) >> 10;
    let mant = (bits as u32) & 0x03FF;
    f32::from_bits(match exp {
        0  => sign,
        31 => sign | 0x7F80_0000 | (mant << 13),
        e  => sign | ((e + 112) << 23) | (mant << 13),
    })
}

pub(super) fn f32_to_f16(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp  = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    let mant = ((bits >> 13) & 0x03FF) as u16;
    if f.is_nan()  { return sign | 0x7E00; }
    if exp <= 0    { return sign; }
    if exp >= 31   { return sign | 0x7C00; }
    sign | ((exp as u16) << 10) | mant
}
