// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`Layer`] struct and [`LayerContent`] enum — the core document model types.
//!
//! [`LayerProp`] and [`PropValue`] are re-exported from `iris-ops` so callers
//! import them via `iris_pixel` without depending on `iris-ops` directly.

use uuid::Uuid;

use crate::blend::BlendMode;
use crate::pixel_layer::PixelLayer;

/// Unique identifier for a layer within a document.
pub type LayerId = Uuid;

/// A single node in the document layer tree.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Unique identifier; set once at creation.
    pub id: LayerId,
    /// Display name shown in the layers panel.
    pub name: String,
    /// Whether the layer contributes to the composite.
    pub visible: bool,
    /// Whether the layer is protected from edits.
    pub locked: bool,
    /// Compositing opacity in the range `0.0` (transparent) to `1.0` (opaque).
    pub opacity: f32,
    /// Compositing blend mode. Only [`BlendMode::Normal`] is wired in Phase 1.
    pub blend_mode: BlendMode,
    /// When `true`, this layer clips to the alpha of the layer directly below it.
    pub clipping_mask: bool,
    /// Optional pixel mask applied non-destructively.
    pub mask: Option<LayerMask>,
    /// The layer's payload — determines whether it is a pixel layer, group, etc.
    pub content: LayerContent,
}

/// The payload of a [`Layer`], distinguishing pixel layers, groups, and future types.
#[derive(Debug, Clone)]
pub enum LayerContent {
    /// A raster pixel layer backed by a tile cache.
    Pixel(PixelLayer),
    /// A group layer whose compositing order is determined by its child list.
    Group {
        /// Ordered list of child layer IDs (index 0 = topmost child).
        children: Vec<LayerId>,
    },
    /// Vector layer — a scene of path objects (SPEC §3.3). Carried for
    /// format round-trips (SVG/AI); GPU compositing is Phase 3+.
    Vector(iris_vector::VectorLayer),
    // TODO(iris): SPEC.md §3.1 — Phase 3: text layer (Parley paragraph)
    /// Text layer — Phase 3 stub; not yet composited.
    Text,
    // TODO(iris): SPEC.md §3.1 — Phase 4: adjustment layer (curves, levels, etc.)
    /// Adjustment layer — Phase 4 stub.
    Adjustment,
    // TODO(iris): SPEC.md §3.1 — Phase 4: solid or gradient fill layer
    /// Fill layer — Phase 4 stub.
    Fill,
    // TODO(iris): SPEC.md §3.1 — Phase 4: smart object (embedded document)
    /// Smart object layer — Phase 4 stub.
    SmartObject,
}

/// Non-destructive pixel mask applied to a layer.
///
/// In the full implementation this wraps a greyscale tile cache used as
/// a per-pixel alpha modifier.
// TODO(iris): SPEC.md §4.6 — Phase 1 stub; fields are defined in SPEC.md §4.6
// which is currently inaccessible (audit §7.4). Expand before iris-aif milestone 2.
#[derive(Debug, Clone)]
pub struct LayerMask;

/// Re-export [`iris_ops::LayerProp`] so callers import from `iris_pixel`.
///
/// `iris-ops` is the canonical owner of this type (audit §6.2 / dep graph).
// TODO(iris): SPEC.md §3 — add BlendMode variant to LayerProp once the
// type-ownership question (audit §7.10) is resolved. Until then, blend mode
// changes are applied directly to Layer.blend_mode and are not undo-tracked.
pub use iris_ops::LayerProp;

/// Re-export [`iris_ops::PropValue`] so callers import from `iris_pixel`.
pub use iris_ops::PropValue;
