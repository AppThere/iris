// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`LayerTree`] — authoritative in-memory document layer tree.

use std::collections::BTreeMap;

use crate::layer::{Layer, LayerId};

mod ops;

/// Internal: where a layer is attached in the tree.
pub(super) enum ParentLocation {
    Root,
    Group(LayerId),
}

/// In-memory document layer tree with canvas metadata.
///
/// All layers live in a flat [`BTreeMap`]; tree structure is encoded by
/// `LayerContent::Group` child-ID lists. Root-level layers are in `root_ids`.
// BTreeMap chosen over HashMap for deterministic iteration order (CLAUDE.md).
#[derive(Debug, Clone)]
pub struct LayerTree {
    /// Canvas width in pixels.
    pub canvas_width: u32,
    /// Canvas height in pixels.
    pub canvas_height: u32,
    /// Horizontal dots-per-inch.
    pub dpi_x: f32,
    /// Vertical dots-per-inch.
    pub dpi_y: f32,
    pub(super) layers: BTreeMap<LayerId, Layer>,
    pub(super) root_ids: Vec<LayerId>,
}

impl LayerTree {
    /// Create an empty layer tree with the given canvas dimensions.
    pub fn new(canvas_width: u32, canvas_height: u32, dpi_x: f32, dpi_y: f32) -> Self {
        Self {
            canvas_width,
            canvas_height,
            dpi_x,
            dpi_y,
            layers: BTreeMap::new(),
            root_ids: Vec::new(),
        }
    }

    /// Ordered slice of root-level layer IDs (index 0 = topmost).
    pub fn root_layer_ids(&self) -> &[LayerId] {
        &self.root_ids
    }

    /// Immutable access to a layer by ID.
    pub fn get(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    /// Mutable access to a layer by ID.
    pub fn get_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }
}

/// Errors produced by [`LayerTree`] mutation operations.
#[derive(Debug, thiserror::Error)]
pub enum LayerTreeError {
    /// No layer with the given ID exists in the tree.
    #[error("layer {0} not found")]
    NotFound(LayerId),
    /// The requested move would create a cycle in the tree.
    #[error("cannot move layer {0} into its own descendant")]
    CircularMove(LayerId),
    /// The requested insertion index exceeds the current child-list length.
    #[error("position {0} out of range")]
    PositionOutOfRange(usize),
}
