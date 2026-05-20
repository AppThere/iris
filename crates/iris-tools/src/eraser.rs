// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`EraserEngine`] — eraser tool implemented as a BrushEngine in destination-out mode.

use iris_canvas::ToolEvent;
use iris_pixel::{Layer, LayerId, TileCoord};

use crate::brush::{BrushEngine, BrushSettings};

/// Eraser tool. Paints transparency (destination-out) over pixels.
///
/// Wraps [`BrushEngine`] with `erase_mode = true` so compositing uses
/// Porter-Duff destination-out rather than over.
#[derive(Clone)]
pub struct EraserEngine {
    inner: BrushEngine,
}

impl EraserEngine {
    /// Create a new eraser with the given diameter (in document pixels).
    pub fn new(size_px: f32) -> Self {
        Self {
            inner: BrushEngine::new(BrushSettings {
                size_px,
                erase_mode: true,
                color: [0.0, 0.0, 0.0, 1.0], // alpha matters; RGB ignored in erase mode
                ..BrushSettings::default()
            }),
        }
    }

    /// Mutable access to the underlying brush settings (size, hardness, opacity).
    pub fn settings_mut(&mut self) -> &mut BrushSettings {
        &mut self.inner.settings
    }

    pub fn on_down(&mut self, event: &ToolEvent, layer: &mut Layer, layer_id: LayerId) {
        self.inner.on_down(event, layer, layer_id);
    }

    pub fn on_move(&mut self, event: &ToolEvent, layer: &mut Layer, layer_id: LayerId) {
        self.inner.on_move(event, layer, layer_id);
    }

    pub fn on_up(&mut self) -> Vec<(LayerId, TileCoord)> {
        self.inner.on_up()
    }
}

impl Default for EraserEngine {
    fn default() -> Self {
        Self::new(20.0)
    }
}
