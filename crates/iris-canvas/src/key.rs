// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`TileKey`] — composite cache key for a specific layer tile.
//!
//! Lives in iris-canvas (not iris-pixel) because it is a rendering concern,
//! not a document-model concern. iris-pixel owns TileCoord and LayerId;
//! iris-canvas owns their combination as a render-cache key.

use iris_pixel::{LayerId, TileCoord};

/// Cache key identifying a specific tile within a specific layer.
///
/// Used by `PageCache<TileKey>` to track hot/warm/cold tier state.
/// Both fields are `Copy`, so `TileKey` is `Copy` and satisfies the
/// `CacheKey` blanket impl (`Hash + Eq + Copy + Send + Sync + 'static`).
///
/// Pan convention: iris-canvas uses screen-center-anchored pan (Q1 decision —
/// top-left was rejected because it is undefined when rotation is non-zero).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    /// The layer this tile belongs to.
    pub layer_id: LayerId,
    /// Tile grid coordinate within that layer.
    pub tile: TileCoord,
}

impl TileKey {
    /// Construct a new key from a layer ID and tile coordinate.
    pub fn new(layer_id: LayerId, tile: TileCoord) -> Self {
        Self { layer_id, tile }
    }
}

// Compile-time assertion: TileKey satisfies the CacheKey blanket impl.
const _: () = {
    fn _assert<T: appthere_canvas::CacheKey>() {}
    fn _check() {
        _assert::<TileKey>();
    }
};
