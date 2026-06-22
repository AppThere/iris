// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Eyedropper tool: sample the composited colour at a document-space point.

use iris_pixel::{LayerContent, LayerTree, TileCoord, TILE_SIZE};

/// Sample the composited colour at `pos` (document space) by compositing all
/// visible pixel layers bottom-to-top with Porter-Duff over.
///
/// Returns linear RGBA as `[f32; 4]` (straight alpha). Returns `[0,0,0,0]`
/// if `pos` is outside all painted tile regions.
pub fn sample_color(pos: kurbo::Vec2, tree: &LayerTree) -> [f32; 4] {
    let mut acc = [0.0_f32; 4]; // premultiplied linear RGBA accumulator

    // iter_depth_first yields top-to-bottom; reverse for bottom-to-top compositing.
    let layers: Vec<_> = tree.iter_depth_first().collect();
    for layer in layers.iter().rev() {
        if !layer.visible { continue; }
        let LayerContent::Pixel(ref px) = layer.content else { continue; };

        let lx = pos.x - px.canvas_offset_x as f64;
        let ly = pos.y - px.canvas_offset_y as f64;
        if lx < 0.0 || ly < 0.0 { continue; }

        let ts = TILE_SIZE as f64;
        let tx = (lx / ts) as u32;
        let ty = (ly / ts) as u32;
        let px_x = (lx % ts) as usize;
        let px_y = (ly % ts) as usize;

        let coord = TileCoord { tx, ty };
        let Some(tile) = px.tiles.get(coord) else { continue; };

        let ts_u = TILE_SIZE as usize;
        let bi = (px_y * ts_u + px_x) * 8;
        let b = tile.bytes();
        if bi + 8 > b.len() { continue; }

        let sr = f16_to_f32(u16::from_le_bytes([b[bi],   b[bi+1]]));
        let sg = f16_to_f32(u16::from_le_bytes([b[bi+2], b[bi+3]]));
        let sb = f16_to_f32(u16::from_le_bytes([b[bi+4], b[bi+5]]));
        let sa = f16_to_f32(u16::from_le_bytes([b[bi+6], b[bi+7]])) * layer.opacity;

        // Porter-Duff over (premultiplied source)
        let inv = 1.0 - sa;
        acc[0] = sr * sa + acc[0] * inv;
        acc[1] = sg * sa + acc[1] * inv;
        acc[2] = sb * sa + acc[2] * inv;
        acc[3] = sa       + acc[3] * inv;
    }

    if acc[3] > f32::EPSILON {
        [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3]]
    } else {
        [0.0, 0.0, 0.0, 0.0]
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use iris_pixel::{
        BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent,
        LayerTree, PixelLayer, TileCache, TileData, TILE_SIZE, LINEAR_SRGB,
    };

    fn tree_with_pixel(px: u32, py: u32, rgba_f16: [u16; 4]) -> LayerTree {
        let mut tree = LayerTree::new(512, 512, 96.0, 96.0);
        let ts = TILE_SIZE as usize;
        let mut tile = TileData::transparent(TILE_SIZE);
        let bi = (py as usize % ts * ts + px as usize % ts) * 8;
        let b = tile.bytes_mut();
        b[bi..bi+2].copy_from_slice(&rgba_f16[0].to_le_bytes());
        b[bi+2..bi+4].copy_from_slice(&rgba_f16[1].to_le_bytes());
        b[bi+4..bi+6].copy_from_slice(&rgba_f16[2].to_le_bytes());
        b[bi+6..bi+8].copy_from_slice(&rgba_f16[3].to_le_bytes());
        let mut cache = TileCache::default();
        cache.insert(TileCoord { tx: px / TILE_SIZE, ty: py / TILE_SIZE }, tile);
        let id = uuid::Uuid::new_v4();
        let layer = Layer {
            id, name: "L".into(), visible: true, locked: false,
            opacity: 1.0, blend_mode: BlendMode::Normal, clipping_mask: false, mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba, bit_depth: BitDepth::F16,
                color_space: LINEAR_SRGB, compression: ExrCompression::Zip,
                canvas_offset_x: 0, canvas_offset_y: 0, crop_bounds: None, tiles: cache,
            }),
        };
        tree.add_layer(None, 0, layer).expect("test setup");
        tree
    }

    #[test]
    fn sample_transparent_returns_zero_alpha() {
        let tree = LayerTree::new(256, 256, 96.0, 96.0);
        let result = sample_color(kurbo::Vec2::new(10.0, 10.0), &tree);
        assert_eq!(result[3], 0.0);
    }

    #[test]
    fn sample_opaque_pixel_returns_color() {
        // f16 1.0 = 0x3C00
        let tree = tree_with_pixel(5, 5, [0x3C00, 0, 0, 0x3C00]); // opaque red
        let result = sample_color(kurbo::Vec2::new(5.5, 5.5), &tree);
        assert!(result[3] > 0.9, "alpha should be ~1.0, got {}", result[3]);
        assert!(result[0] > 0.9, "red should be ~1.0, got {}", result[0]);
        assert!(result[1] < 0.1, "green should be ~0.0");
    }
}
