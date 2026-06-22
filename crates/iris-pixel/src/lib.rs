// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Raster document model: in-memory layer tree with pixel tile storage.
//!
//! See ADR/001-pixel-vector-unified-canvas.md, ADR/003-color-pipeline.md,
//! and `crates/iris-pixel/BRIEF.md` before implementing.
//!
//! **Phase 1:** layer tree, pixel tile cache, blend mode enum.
//! **Phase 3:** vector and text layer content.
//! **Phase 4:** adjustment layers, fill layers, CMYK, smart objects.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod blend;
mod color_space;
mod layer;
mod pixel_layer;
mod tile;
mod tile_data;
mod tree;

pub use blend::BlendMode;
pub use color_space::{ColorSpaceId, CMYK_GENERIC, DISPLAY_P3, LINEAR_SRGB, PROPHOTO_RGB, SRGB};
pub use layer::{Layer, LayerContent, LayerId, LayerMask, LayerProp, PropValue};
pub use pixel_layer::{BitDepth, ChannelLayout, CropBounds, ExrCompression, PixelLayer};
pub use tile::{TileCache, TileCoord, TILE_SIZE};
pub use tile_data::TileData;
pub use tree::{LayerTree, LayerTreeError};

/// Re-export the vector document model so callers that hold a [`LayerTree`]
/// can construct [`LayerContent::Vector`] payloads without a separate import.
pub use iris_vector::VectorLayer;
