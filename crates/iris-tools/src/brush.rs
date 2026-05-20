// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`BrushEngine`] — hard round brush with pressure-sensitive size and
//! sub-pixel spacing interpolation.

use std::collections::HashSet;

use iris_canvas::ToolEvent;
use iris_pixel::{Layer, LayerContent, LayerId, PixelLayer, TileCoord, TileData, TILE_SIZE};

/// Brush configuration. All fields are public so the UI can bind directly.
#[derive(Debug, Clone)]
pub struct BrushSettings {
    /// Diameter in document pixels (1.0 – 2000.0).
    pub size_px: f32,
    /// Layer opacity multiplier (0.0 – 1.0).
    pub opacity: f32,
    /// Edge falloff: 1.0 = fully hard, 0.0 = fully soft.
    pub hardness: f32,
    /// Stamp spacing as a fraction of diameter (0.05 – 1.0).
    pub spacing: f32,
    /// Linear RGBA brush colour.
    pub color: [f32; 4],
    /// When `true`, pressure scales the effective diameter.
    pub pressure_size: bool,
    /// When `true`, compositing uses destination-out (erase) instead of over.
    pub erase_mode: bool,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            size_px: 20.0,
            opacity: 1.0,
            hardness: 0.8,
            spacing: 0.1,
            color: [0.0, 0.0, 0.0, 1.0],
            pressure_size: true,
            erase_mode: false,
        }
    }
}

/// Stateful brush that converts [`ToolEvent`]s into painted [`TileData`] mutations.
#[derive(Clone)]
pub struct BrushEngine {
    pub settings: BrushSettings,
    last_pos: Option<kurbo::Vec2>,
    dist_acc: f64,
    dirty_tiles: HashSet<(LayerId, TileCoord)>,
}

impl BrushEngine {
    pub fn new(settings: BrushSettings) -> Self {
        Self { settings, last_pos: None, dist_acc: 0.0, dirty_tiles: HashSet::new() }
    }

    /// Begin a new stroke. Stamps the brush at the down position.
    pub fn on_down(
        &mut self,
        event: &ToolEvent,
        layer: &mut Layer,
        layer_id: LayerId,
    ) {
        self.last_pos = None;
        self.dist_acc = 0.0;
        self.dirty_tiles.clear();
        if let ToolEvent::Down { doc_pos, pressure, .. } = event {
            self.stamp(*doc_pos, *pressure, layer, layer_id);
        }
    }

    /// Continue a stroke. Interpolates stamps along the path since last position.
    pub fn on_move(
        &mut self,
        event: &ToolEvent,
        layer: &mut Layer,
        layer_id: LayerId,
    ) {
        if let ToolEvent::Move { doc_pos, pressure, .. } = event {
            self.interpolate(*doc_pos, *pressure, layer, layer_id);
        }
    }

    /// End the stroke. Returns the set of dirtied (layer_id, tile_coord) for undo.
    pub fn on_up(&mut self) -> Vec<(LayerId, TileCoord)> {
        self.last_pos = None;
        self.dirty_tiles.drain().collect()
    }

    /// Place a single brush stamp at `pos` with the given pressure.
    fn stamp(
        &mut self,
        pos: kurbo::Vec2,
        pressure: f32,
        layer: &mut Layer,
        layer_id: LayerId,
    ) {
        let size = if self.settings.pressure_size {
            self.settings.size_px * pressure
        } else {
            self.settings.size_px
        };
        let radius = (size / 2.0).max(0.5) as f64;

        let pixel_layer = match &mut layer.content {
            LayerContent::Pixel(pl) => pl,
            _ => return,
        };

        let bbox_x0 = (pos.x - radius) as i64;
        let bbox_y0 = (pos.y - radius) as i64;
        let bbox_x1 = (pos.x + radius) as i64 + 1;
        let bbox_y1 = (pos.y + radius) as i64 + 1;

        let off_x = pixel_layer.canvas_offset_x as i64;
        let off_y = pixel_layer.canvas_offset_y as i64;
        let ts = TILE_SIZE as i64;

        let tx0 = (bbox_x0 - off_x).div_euclid(ts) as i32;
        let ty0 = (bbox_y0 - off_y).div_euclid(ts) as i32;
        let tx1 = (bbox_x1 - off_x).div_euclid(ts) as i32;
        let ty1 = (bbox_y1 - off_y).div_euclid(ts) as i32;

        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                if tx < 0 || ty < 0 { continue; }
                let coord = TileCoord { tx: tx as u32, ty: ty as u32 };
                self.paint_tile(pos, radius, coord, pixel_layer, layer_id);
            }
        }
        self.last_pos = Some(pos);
    }

    /// Paint the brush circle onto one tile.
    fn paint_tile(
        &mut self,
        center: kurbo::Vec2,
        radius: f64,
        coord: TileCoord,
        pl: &mut PixelLayer,
        layer_id: LayerId,
    ) {
        let mut tile = pl.tiles.get(coord).cloned()
            .unwrap_or_else(|| TileData::transparent(TILE_SIZE));

        let tile_ox = pl.canvas_offset_x as f64 + coord.tx as f64 * TILE_SIZE as f64;
        let tile_oy = pl.canvas_offset_y as f64 + coord.ty as f64 * TILE_SIZE as f64;
        let color = self.settings.color;
        let opacity = self.settings.opacity;
        let hardness = self.settings.hardness;
        let erase = self.settings.erase_mode;
        let ts = TILE_SIZE as usize;

        {
            let bytes = &mut tile.0;
            for py in 0..ts {
                for px in 0..ts {
                    let wx = tile_ox + px as f64 + 0.5;
                    let wy = tile_oy + py as f64 + 0.5;
                    let dist = ((wx - center.x).powi(2) + (wy - center.y).powi(2)).sqrt();
                    if dist > radius { continue; }

                    let alpha = {
                        let falloff_start = radius * hardness as f64;
                        if hardness >= 1.0 || dist <= falloff_start {
                            opacity
                        } else {
                            let t = ((dist - falloff_start) / (radius - falloff_start)) as f32;
                            opacity * (1.0 - t)
                        }
                    };

                    let bi = (py * ts + px) * 8;
                    let dst_r = f16_to_f32(u16::from_le_bytes([bytes[bi],     bytes[bi+1]]));
                    let dst_g = f16_to_f32(u16::from_le_bytes([bytes[bi+2],   bytes[bi+3]]));
                    let dst_b = f16_to_f32(u16::from_le_bytes([bytes[bi+4],   bytes[bi+5]]));
                    let dst_a = f16_to_f32(u16::from_le_bytes([bytes[bi+6],   bytes[bi+7]]));

                    let (out_r, out_g, out_b, out_a) = if erase {
                        // Destination-out: scale dst down by brush contribution.
                        let f = 1.0 - alpha;
                        (dst_r * f, dst_g * f, dst_b * f, dst_a * f)
                    } else {
                        // Porter-Duff over (premultiplied src).
                        let src_a = color[3] * alpha;
                        let f = 1.0 - src_a;
                        (color[0] * src_a + dst_r * f,
                         color[1] * src_a + dst_g * f,
                         color[2] * src_a + dst_b * f,
                         src_a + dst_a * f)
                    };

                    bytes[bi..bi+2].copy_from_slice(&f32_to_f16(out_r).to_le_bytes());
                    bytes[bi+2..bi+4].copy_from_slice(&f32_to_f16(out_g).to_le_bytes());
                    bytes[bi+4..bi+6].copy_from_slice(&f32_to_f16(out_b).to_le_bytes());
                    bytes[bi+6..bi+8].copy_from_slice(&f32_to_f16(out_a).to_le_bytes());
                }
            }
        }

        pl.tiles.insert(coord, tile);
        self.dirty_tiles.insert((layer_id, coord));
    }

    /// Walk from `last_pos` to `new_pos`, stamping at `spacing` intervals.
    fn interpolate(
        &mut self,
        new_pos: kurbo::Vec2,
        pressure: f32,
        layer: &mut Layer,
        layer_id: LayerId,
    ) {
        let Some(last) = self.last_pos else {
            self.stamp(new_pos, pressure, layer, layer_id);
            return;
        };
        let spacing = (self.settings.size_px as f64 * self.settings.spacing as f64).max(1.0);
        let delta = new_pos - last;
        let seg_len = delta.hypot();
        if seg_len < 1e-6 { return; }

        let mut covered = spacing - self.dist_acc;
        while covered <= seg_len {
            let t = covered / seg_len;
            self.stamp(last + delta * t, pressure, layer, layer_id);
            covered += spacing;
        }
        self.dist_acc = seg_len - (covered - spacing);
        self.last_pos = Some(new_pos);
    }
}

impl Default for BrushEngine {
    fn default() -> Self {
        Self::new(BrushSettings::default())
    }
}

/// Convert IEEE 754 half-precision bits to f32.
/// Subnormal f16 values are treated as zero — adequate for pixel data.
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

/// Encode an f32 value to IEEE 754 half-precision bits (truncate, no round).
fn f32_to_f16(f: f32) -> u16 {
    let bits = f.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp  = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    let mant = ((bits >> 13) & 0x03FF) as u16;
    if f.is_nan()      { return sign | 0x7E00; }
    if exp <= 0        { return sign; }
    if exp >= 31       { return sign | 0x7C00; }
    sign | ((exp as u16) << 10) | mant
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_pixel::{BitDepth, BlendMode, ChannelLayout, ExrCompression, TileCache, LINEAR_SRGB};

    fn make_pixel_layer() -> iris_pixel::PixelLayer {
        iris_pixel::PixelLayer {
            channel_layout: ChannelLayout::Rgba,
            bit_depth: BitDepth::F16,
            color_space: LINEAR_SRGB,
            compression: ExrCompression::Zip,
            canvas_offset_x: 0,
            canvas_offset_y: 0,
            crop_bounds: None,
            tiles: TileCache::default(),
        }
    }

    fn make_layer() -> (Layer, LayerId) {
        let id = uuid::Uuid::new_v4();
        let layer = Layer {
            id,
            name: "Test".into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            clipping_mask: false,
            mask: None,
            content: LayerContent::Pixel(make_pixel_layer()),
        };
        (layer, id)
    }

    fn alpha_at(tile: &TileData, px: usize, py: usize) -> f32 {
        let bi = (py * TILE_SIZE as usize + px) * 8;
        f16_to_f32(u16::from_le_bytes([tile.0[bi+6], tile.0[bi+7]]))
    }

    #[test]
    fn stamp_paints_pixels_within_radius() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 10.0, opacity: 1.0, hardness: 1.0,
            pressure_size: false, ..BrushSettings::default()
        });
        let event = ToolEvent::Down {
            doc_pos: kurbo::Vec2::new(128.0, 128.0),
            pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0,
            button: iris_canvas::PointerButton::Primary,
        };
        engine.on_down(&event, &mut layer, id);

        let coord = TileCoord { tx: 0, ty: 0 };
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(coord).expect("tile created");
        // Center pixel (128,128) should be painted
        assert!(alpha_at(tile, 128, 128) > 0.9, "center pixel painted");
        // Pixel far outside radius (0,0) should be untouched
        assert_eq!(alpha_at(tile, 0, 0), 0.0, "outside pixel untouched");
    }

    #[test]
    fn eraser_clears_pixels() {
        let (mut layer, id) = make_layer();
        // Paint white first
        let mut brush = BrushEngine::new(BrushSettings {
            size_px: 20.0, opacity: 1.0, hardness: 1.0,
            color: [1.0, 1.0, 1.0, 1.0], pressure_size: false,
            ..BrushSettings::default()
        });
        let down = ToolEvent::Down {
            doc_pos: kurbo::Vec2::new(128.0, 128.0),
            pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0,
            button: iris_canvas::PointerButton::Primary,
        };
        brush.on_down(&down, &mut layer, id);

        // Erase
        let mut eraser = BrushEngine::new(BrushSettings {
            size_px: 20.0, opacity: 1.0, hardness: 1.0,
            erase_mode: true, pressure_size: false, ..BrushSettings::default()
        });
        eraser.on_down(&down, &mut layer, id);

        let coord = TileCoord { tx: 0, ty: 0 };
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(coord).expect("tile exists");
        assert!(alpha_at(tile, 128, 128) < 0.05, "pixel erased to transparent");
    }

    #[test]
    fn pressure_half_shrinks_brush() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 40.0, opacity: 1.0, hardness: 1.0,
            pressure_size: true, ..BrushSettings::default()
        });
        // pressure=0.5 → effective radius = 10
        let event = ToolEvent::Down {
            doc_pos: kurbo::Vec2::new(128.0, 128.0),
            pressure: 0.5, tilt_x: 0.0, tilt_y: 0.0,
            button: iris_canvas::PointerButton::Primary,
        };
        engine.on_down(&event, &mut layer, id);

        let coord = TileCoord { tx: 0, ty: 0 };
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(coord).expect("tile created");
        // pixel 12 away from center should be outside effective radius 10
        assert_eq!(alpha_at(tile, 128 + 12, 128), 0.0, "outside half-pressure radius");
        // pixel at center painted
        assert!(alpha_at(tile, 128, 128) > 0.9, "center painted");
    }

    #[test]
    fn interpolate_places_multiple_stamps() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 10.0, opacity: 1.0, hardness: 1.0,
            spacing: 0.5, pressure_size: false, ..BrushSettings::default()
        });
        let down = ToolEvent::Down {
            doc_pos: kurbo::Vec2::new(10.0, 128.0),
            pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0,
            button: iris_canvas::PointerButton::Primary,
        };
        engine.on_down(&down, &mut layer, id);
        // Move 100px to the right — spacing=5px → ~20 stamps
        let move_ev = ToolEvent::Move {
            doc_pos: kurbo::Vec2::new(110.0, 128.0),
            pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0,
        };
        engine.on_move(&move_ev, &mut layer, id);
        let dirty = engine.on_up();
        // At least one tile must be dirty
        assert!(!dirty.is_empty(), "stroke left dirty tiles");
    }
}
