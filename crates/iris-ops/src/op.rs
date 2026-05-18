// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Typed `Op` enum and all associated data types for the iris-ops undo/redo engine.

use uuid::Uuid;

use crate::snapshot::TileSnapshot;

/// Top-level document operation recorded in the undo/redo stack.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// A mutation to the layer tree structure or layer scalar properties.
    Layer(LayerOp),
    /// A mutation to pixel tile data.
    Tile(TileOp),
}

/// Layer-tree mutation variants.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerOp {
    /// Add a new layer to the document tree.
    Add {
        /// UUID of the parent layer, or `None` to insert at the document root.
        parent: Option<Uuid>,
        /// Insertion index within the parent's child list (0 = top).
        position: usize,
        /// Compact serialised metadata sufficient to reconstruct the layer on undo.
        snapshot: LayerSnapshot,
    },
    /// Remove a layer from the document tree.
    Remove {
        /// UUID of the layer being removed.
        layer_id: Uuid,
        /// Snapshot taken immediately before removal; used to restore the layer on undo.
        snapshot: LayerSnapshot,
    },
    /// Move a layer to a different parent or position.
    Move {
        /// UUID of the layer being moved.
        layer_id: Uuid,
        /// Destination parent UUID, or `None` for the document root.
        new_parent: Option<Uuid>,
        /// Index within the destination parent's child list after the move.
        new_position: usize,
        /// Previous parent UUID, or `None` if the layer was at the document root.
        old_parent: Option<Uuid>,
        /// Previous index within the old parent's child list.
        old_position: usize,
    },
    /// Change a scalar property on a layer.
    SetProp {
        /// UUID of the layer whose property changed.
        layer_id: Uuid,
        /// Which property changed.
        prop: LayerProp,
        /// Property value before the change; applied on undo.
        before: PropValue,
        /// Property value after the change; applied on redo.
        after: PropValue,
    },
}

/// Pixel-data mutation variants.
#[derive(Debug, Clone, PartialEq)]
pub enum TileOp {
    /// Overwrite a tile with new pixel data (e.g. after a brush stroke).
    Paint {
        /// UUID of the layer containing the tile.
        layer_id: Uuid,
        /// Grid coordinate of the tile being modified.
        tile: TileCoord,
        /// Pixel data before the stroke; restored on undo.
        before: TileSnapshot,
        /// Pixel data after the stroke; restored on redo.
        after: TileSnapshot,
    },
}

/// Compact serialised form of a layer's metadata, sufficient to reconstruct the
/// layer on undo. Does **not** contain pixel tile data.
///
/// The encoding is opaque to `iris-ops`. Callers (e.g. `iris-pixel`) serialise
/// their `Layer` struct into bytes and pass those bytes here. This keeps `iris-ops`
/// free of any dependency on `iris-pixel` or `iris-vector`.
// TODO(iris): SPEC.md §3 — stabilise the binary encoding once iris-pixel is
// complete and the type-ownership question from audit §6.3 is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerSnapshot(Vec<u8>);

impl LayerSnapshot {
    /// Wrap raw serialised layer-metadata bytes into a snapshot.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the raw serialised bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Integer tile-grid coordinate identifying one 256 × 256 pixel tile.
///
/// `(tx, ty)` addresses the tile whose top-left pixel is at canvas position
/// `(tx * 256, ty * 256)`.
// TODO(iris): SPEC.md §3 — confirm canonical ownership. iris-pixel/BRIEF.md also
// defines TileCoord; once iris-pixel is implemented it should re-export this type
// from iris-ops (audit §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    /// Horizontal tile index (column).
    pub tx: u32,
    /// Vertical tile index (row).
    pub ty: u32,
}

/// Identifies which scalar property of a layer changed in a [`LayerOp::SetProp`].
// TODO(iris): SPEC.md §3 — add `BlendMode` variant once the type-ownership
// question from audit §6.2 is resolved (BlendMode is currently owned by
// iris-pixel, which depends on iris-ops, not the reverse).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerProp {
    /// The layer's display name (`Layer.name: String`).
    Name,
    /// Whether the layer is visible in the compositor (`Layer.visible: bool`).
    Visible,
    /// Whether the layer is locked against further edits (`Layer.locked: bool`).
    Locked,
    /// Compositing opacity in the range 0.0–1.0 (`Layer.opacity: f32`).
    Opacity,
}

/// A typed scalar value paired with a [`LayerProp`] in a [`LayerOp::SetProp`].
// TODO(iris): SPEC.md §3 — add a `BlendMode` variant once audit §6.2 is resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    /// A string value (e.g. layer name).
    Str(String),
    /// A boolean value (e.g. `visible`, `locked`).
    Bool(bool),
    /// A floating-point value (e.g. `opacity`).
    Float(f32),
}
