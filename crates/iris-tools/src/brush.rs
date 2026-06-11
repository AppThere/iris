// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`BrushEngine`] — hard round brush with pressure-sensitive size and
//! sub-pixel spacing interpolation.

use std::collections::HashSet;

use iris_canvas::ToolEvent;
use iris_pixel::{Layer, LayerId, TileCoord};

mod paint;

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
    /// Active marquee selection. When `Some`, only pixels inside the rect are painted.
    pub selection: Option<kurbo::Rect>,
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
            selection: None,
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
    pub fn on_down(&mut self, event: &ToolEvent, layer: &mut Layer, layer_id: LayerId) {
        self.last_pos = None;
        self.dist_acc = 0.0;
        self.dirty_tiles.clear();
        if let ToolEvent::Down { doc_pos, pressure, .. } = event {
            paint::stamp(
                &mut self.last_pos, &mut self.dirty_tiles,
                &self.settings, *doc_pos, *pressure, layer, layer_id,
            );
        }
    }

    /// Continue a stroke. Interpolates stamps along the path since last position.
    pub fn on_move(&mut self, event: &ToolEvent, layer: &mut Layer, layer_id: LayerId) {
        if let ToolEvent::Move { doc_pos, pressure, .. } = event {
            paint::interpolate(
                &mut self.last_pos, &mut self.dist_acc, &mut self.dirty_tiles,
                &self.settings, *doc_pos, *pressure, layer, layer_id,
            );
        }
    }

    /// End the stroke. Returns the set of dirtied (layer_id, tile_coord) for undo.
    pub fn on_up(&mut self) -> Vec<(LayerId, TileCoord)> {
        self.last_pos = None;
        self.dirty_tiles.drain().collect()
    }
}

impl Default for BrushEngine {
    fn default() -> Self { Self::new(BrushSettings::default()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_canvas::PointerButton;
    use iris_pixel::{
        BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent,
        PixelLayer, TileCache, TileCoord, TileData, TILE_SIZE, LINEAR_SRGB,
    };

    fn make_layer() -> (Layer, LayerId) {
        let id = uuid::Uuid::new_v4();
        let layer = Layer {
            id, name: "Test".into(), visible: true, locked: false,
            opacity: 1.0, blend_mode: BlendMode::Normal, clipping_mask: false, mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba, bit_depth: BitDepth::F16,
                color_space: LINEAR_SRGB, compression: ExrCompression::Zip,
                canvas_offset_x: 0, canvas_offset_y: 0,
                crop_bounds: None, tiles: TileCache::default(),
            }),
        };
        (layer, id)
    }

    fn alpha_at(tile: &TileData, px: usize, py: usize) -> f32 {
        let bi = (py * TILE_SIZE as usize + px) * 8;
        paint::f16_to_f32(u16::from_le_bytes([tile.bytes()[bi+6], tile.bytes()[bi+7]]))
    }

    fn down_at(x: f64, y: f64) -> ToolEvent {
        ToolEvent::Down {
            doc_pos: kurbo::Vec2::new(x, y), pressure: 1.0,
            tilt_x: 0.0, tilt_y: 0.0, button: PointerButton::Primary,
        }
    }

    #[test]
    fn stamp_paints_pixels_within_radius() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 10.0, opacity: 1.0, hardness: 1.0, pressure_size: false,
            ..BrushSettings::default()
        });
        engine.on_down(&down_at(128.0, 128.0), &mut layer, id);
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(TileCoord { tx: 0, ty: 0 }).expect("tile created");
        assert!(alpha_at(tile, 128, 128) > 0.9, "center pixel painted");
        assert_eq!(alpha_at(tile, 0, 0), 0.0, "outside pixel untouched");
    }

    /// Painting goes through TileCache::get_mut + TileData::bytes_mut, which
    /// is copy-on-write: a layer clone taken between strokes (the render
    /// snapshot pattern) must never see later paint operations.
    #[test]
    fn render_snapshot_unaffected_by_later_painting() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 10.0, opacity: 1.0, hardness: 1.0, pressure_size: false,
            ..BrushSettings::default()
        });
        engine.on_down(&down_at(128.0, 128.0), &mut layer, id);
        let snapshot = layer.clone(); // cheap: tiles are Arc-shared

        engine.on_down(&down_at(20.0, 20.0), &mut layer, id);

        let LayerContent::Pixel(ref pl) = snapshot.content else { panic!() };
        let tile = pl.tiles.get(TileCoord { tx: 0, ty: 0 }).expect("tile in snapshot");
        assert!(alpha_at(tile, 128, 128) > 0.9, "snapshot keeps first stroke");
        assert_eq!(alpha_at(tile, 20, 20), 0.0, "snapshot must not see later stroke");

        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(TileCoord { tx: 0, ty: 0 }).expect("live tile");
        assert!(alpha_at(tile, 20, 20) > 0.9, "live layer has later stroke");
    }

    #[test]
    fn eraser_clears_pixels() {
        let (mut layer, id) = make_layer();
        let mut brush = BrushEngine::new(BrushSettings {
            size_px: 20.0, opacity: 1.0, hardness: 1.0,
            color: [1.0, 1.0, 1.0, 1.0], pressure_size: false, ..BrushSettings::default()
        });
        brush.on_down(&down_at(128.0, 128.0), &mut layer, id);
        let mut eraser = BrushEngine::new(BrushSettings {
            size_px: 20.0, opacity: 1.0, hardness: 1.0,
            erase_mode: true, pressure_size: false, ..BrushSettings::default()
        });
        eraser.on_down(&down_at(128.0, 128.0), &mut layer, id);
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(TileCoord { tx: 0, ty: 0 }).expect("tile exists");
        assert!(alpha_at(tile, 128, 128) < 0.05, "pixel erased to transparent");
    }

    #[test]
    fn pressure_half_shrinks_brush() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 40.0, opacity: 1.0, hardness: 1.0, pressure_size: true,
            ..BrushSettings::default()
        });
        let event = ToolEvent::Down {
            doc_pos: kurbo::Vec2::new(128.0, 128.0), pressure: 0.5,
            tilt_x: 0.0, tilt_y: 0.0, button: PointerButton::Primary,
        };
        engine.on_down(&event, &mut layer, id);
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        let tile = pl.tiles.get(TileCoord { tx: 0, ty: 0 }).expect("tile created");
        assert_eq!(alpha_at(tile, 128 + 12, 128), 0.0, "outside half-pressure radius");
        assert!(alpha_at(tile, 128, 128) > 0.9, "center painted");
    }

    #[test]
    fn interpolate_places_multiple_stamps() {
        let (mut layer, id) = make_layer();
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 10.0, opacity: 1.0, hardness: 1.0, spacing: 0.5, pressure_size: false,
            ..BrushSettings::default()
        });
        engine.on_down(&down_at(10.0, 128.0), &mut layer, id);
        let move_ev = ToolEvent::Move {
            doc_pos: kurbo::Vec2::new(110.0, 128.0), pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0,
        };
        engine.on_move(&move_ev, &mut layer, id);
        assert!(!engine.on_up().is_empty(), "stroke left dirty tiles");
    }

    #[test]
    fn selection_restricts_brush_painting() {
        let (mut layer, id) = make_layer();
        // Selection covers only x in [200, 400): center at (128, 128) is outside.
        let mut engine = BrushEngine::new(BrushSettings {
            size_px: 40.0, opacity: 1.0, hardness: 1.0, pressure_size: false,
            selection: Some(kurbo::Rect::new(200.0, 0.0, 400.0, 400.0)),
            ..BrushSettings::default()
        });
        engine.on_down(&down_at(128.0, 128.0), &mut layer, id);
        let LayerContent::Pixel(ref pl) = layer.content else { panic!() };
        if let Some(tile) = pl.tiles.get(TileCoord { tx: 0, ty: 0 }) {
            assert_eq!(alpha_at(tile, 128, 128), 0.0, "pixel outside selection not painted");
        }
    }
}
