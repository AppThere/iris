// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel tile primitives: [`TileData`] (raw f16 RGBA bytes) and [`TileCache`]
//! (capacity-bounded map with FIFO eviction and dirty tracking).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

/// Canonical tile size in pixels along each edge (256 × 256).
pub const TILE_SIZE: u32 = 256;

/// Re-export the canonical [`TileCoord`] type from `iris-ops` so callers
/// import from `iris_pixel` rather than `iris_ops` directly.
// TODO(iris): SPEC.md §3 — TileCoord is defined in iris-ops (audit §6.4).
// iris-pixel re-exports it here so callers use iris_pixel::TileCoord.
pub use iris_ops::TileCoord;

/// Raw f16 RGBA pixel data for one tile, shared copy-on-write.
///
/// Layout: `tile_size × tile_size × 4 channels × 2 bytes (f16)`.
/// For the default [`TILE_SIZE`] of 256 this is 524 288 bytes per tile.
///
/// The buffer is behind an `Arc`: cloning a `TileData` (and therefore a
/// [`TileCache`] or a whole `LayerTree`) only bumps a reference count, so the
/// per-event tree snapshots that sync app state to the render thread cost
/// O(tiles) pointer copies instead of O(pixels) memcpy. The first
/// [`bytes_mut`][Self::bytes_mut] on a shared tile deep-copies that one tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileData(Arc<Vec<u8>>);

impl TileData {
    /// Wrap an existing pixel buffer.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        TileData(Arc::new(bytes))
    }

    /// Allocate a fully-transparent tile (all channels = f16 `0.0`).
    ///
    /// `tile_size` is the edge length in pixels; use [`TILE_SIZE`] for the
    /// default 256-pixel tile.
    pub fn transparent(tile_size: u32) -> Self {
        let byte_count = (tile_size as usize)
            .saturating_mul(tile_size as usize)
            .saturating_mul(4)   // RGBA channels
            .saturating_mul(2);  // bytes per f16
        Self::from_vec(vec![0u8; byte_count])
    }

    /// Read access to the raw pixel bytes (zero-copy; used by `iris-aif`'s
    /// OpenEXR encoder and the compositor sampling loops).
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// Mutable access to the raw pixel bytes.
    ///
    /// Copy-on-write: if this buffer is shared with another `TileData` clone
    /// (e.g. a render-thread snapshot), it is deep-copied once before the
    /// mutable borrow is handed out. Hoist this call out of per-pixel loops.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        Arc::make_mut(&mut self.0).as_mut_slice()
    }

    /// Returns `true` if every pixel's alpha channel is f16 `0.0`.
    ///
    /// f16 RGBA layout: each pixel is 8 bytes — R(2) G(2) B(2) A(2).
    /// Alpha occupies bytes 6–7 of each 8-byte group; f16 zero = `[0x00, 0x00]`.
    pub fn is_fully_transparent(&self) -> bool {
        self.0.chunks_exact(8).all(|px| px[6] == 0 && px[7] == 0)
    }

    /// Return the number of bytes in this tile's raw pixel buffer.
    pub fn byte_len(&self) -> usize {
        self.0.len()
    }
}

/// Capacity of a [`TileCache`] created with [`TileCache::default`].
pub const DEFAULT_TILE_CACHE_CAPACITY: usize = 512;

/// Capacity-bounded map from [`TileCoord`] to [`TileData`] with FIFO eviction
/// and dirty-tile tracking.
///
/// Eviction strategy: when the cache is full, the tile that was **inserted
/// earliest** is dropped (FIFO). On eviction of a dirty tile a `tracing::warn!`
/// is emitted — Phase 1 has no flush path, so callers must size the cache to
/// fit the working set or accept data loss on overflow.
// TODO(iris): SPEC.md §4.7 — Phase 1 milestone 2: flush dirty tiles to OpenEXR
// before eviction once iris-aif is available.
#[derive(Debug, Clone)]
pub struct TileCache {
    data: BTreeMap<TileCoord, TileData>,
    /// Insertion order for FIFO eviction (front = oldest).
    order: VecDeque<TileCoord>,
    dirty: BTreeSet<TileCoord>,
    capacity: usize,
}

impl TileCache {
    /// Create a new cache with the given tile capacity.
    ///
    /// `capacity = 0` is valid but means every insert immediately evicts the
    /// previous tile. Use [`DEFAULT_TILE_CACHE_CAPACITY`] for a typical
    /// in-memory working set.
    pub fn new(capacity: usize) -> Self {
        Self {
            data: BTreeMap::new(),
            order: VecDeque::new(),
            dirty: BTreeSet::new(),
            capacity,
        }
    }

    /// Look up the tile at `coord`. Returns `None` if the tile is not cached.
    pub fn get(&self, coord: TileCoord) -> Option<&TileData> {
        self.data.get(&coord)
    }

    /// Mutable lookup; marks the tile dirty (callers borrow it to paint).
    ///
    /// Prefer this over `get(..).cloned()` + [`insert`][Self::insert] in paint
    /// loops: combined with [`TileData::bytes_mut`]'s copy-on-write it avoids
    /// a 512 KB tile copy per brush dab.
    pub fn get_mut(&mut self, coord: TileCoord) -> Option<&mut TileData> {
        let tile = self.data.get_mut(&coord)?;
        self.dirty.insert(coord);
        Some(tile)
    }

    /// Insert or overwrite the tile at `coord` and mark it dirty.
    ///
    /// If the cache is already at capacity and `coord` is a new entry, the
    /// oldest tile is evicted first. Updating an existing coord does not
    /// change its eviction order.
    pub fn insert(&mut self, coord: TileCoord, data: TileData) {
        if let std::collections::btree_map::Entry::Occupied(mut e) = self.data.entry(coord) {
            // Update in place; eviction position in `order` is unchanged.
            e.insert(data);
            self.dirty.insert(coord);
            return;
        }
        // Evict the oldest tile if the cache is full.
        if self.data.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                if self.dirty.contains(&evicted) {
                    // Phase 1 has no disk-flush path; warn rather than silently losing data.
                    tracing::warn!(
                        coord = ?evicted,
                        "TileCache evicting dirty tile without flush (Phase 1 limitation)"
                    );
                }
                self.data.remove(&evicted);
                self.dirty.remove(&evicted);
            }
        }
        self.order.push_back(coord);
        self.data.insert(coord, data);
        self.dirty.insert(coord);
    }

    /// Remove the tile at `coord` from the cache, returning it if present.
    pub fn remove(&mut self, coord: TileCoord) -> Option<TileData> {
        let removed = self.data.remove(&coord)?;
        self.dirty.remove(&coord);
        // O(n) removal from VecDeque — acceptable for Phase 1 cache sizes.
        self.order.retain(|&c| c != coord);
        Some(removed)
    }

    /// Iterate over all coordinates that have been modified since last flush.
    pub fn dirty_coords(&self) -> impl Iterator<Item = TileCoord> + '_ {
        self.dirty.iter().copied()
    }

    /// Mark `coord` as clean (e.g. after flushing to disk).
    pub fn mark_clean(&mut self, coord: TileCoord) {
        self.dirty.remove(&coord);
    }
}

impl Default for TileCache {
    fn default() -> Self {
        Self::new(DEFAULT_TILE_CACHE_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_tile(value: u8) -> TileData {
        TileData::from_vec(vec![value; TILE_SIZE as usize * TILE_SIZE as usize * 8])
    }

    #[test]
    fn clone_shares_buffer_until_mutation() {
        let mut a = solid_tile(1);
        let b = a.clone();
        assert_eq!(a.bytes().as_ptr(), b.bytes().as_ptr(), "clone must share");
        a.bytes_mut()[0] = 9;
        assert_ne!(a.bytes().as_ptr(), b.bytes().as_ptr(), "write must un-share");
        assert_eq!(b.bytes()[0], 1, "snapshot clone must be unaffected");
        assert_eq!(a.bytes()[0], 9);
    }

    #[test]
    fn get_mut_marks_dirty() {
        let mut cache = TileCache::new(16);
        let coord = TileCoord { tx: 1, ty: 2 };
        cache.insert(coord, solid_tile(1));
        cache.mark_clean(coord);
        assert_eq!(cache.dirty_coords().count(), 0);
        assert!(cache.get_mut(coord).is_some());
        assert_eq!(cache.dirty_coords().count(), 1, "get_mut must mark dirty");
    }

    #[test]
    fn transparent_tile_is_fully_transparent() {
        let tile = TileData::transparent(TILE_SIZE);
        assert!(tile.is_fully_transparent());
    }

    #[test]
    fn painted_tile_is_not_transparent() {
        // Alpha bytes (indices 6 and 7 of each 8-byte group) non-zero → not transparent.
        let mut bytes = vec![0u8; 8];
        bytes[6] = 0xFF; // alpha = f16 large value (approximately 0.0001 in f16)
        bytes[7] = 0x00;
        let tile = TileData::from_vec(bytes);
        assert!(!tile.is_fully_transparent());
    }

    #[test]
    fn non_zero_rgb_with_zero_alpha_is_still_transparent() {
        // Only alpha bytes matter for transparency; RGB may be non-zero.
        let mut bytes = vec![0u8; 8];
        bytes[0] = 0xFF; // R channel non-zero
        bytes[6] = 0;    // alpha = 0
        bytes[7] = 0;
        let tile = TileData::from_vec(bytes);
        assert!(tile.is_fully_transparent());
    }

    #[test]
    fn tile_cache_lru_eviction_drops_oldest() {
        let mut cache = TileCache::new(2);
        let c00 = TileCoord { tx: 0, ty: 0 };
        let c01 = TileCoord { tx: 0, ty: 1 };
        let c02 = TileCoord { tx: 0, ty: 2 };
        cache.insert(c00, solid_tile(1));
        cache.insert(c01, solid_tile(2));
        cache.insert(c02, solid_tile(3)); // evicts c00
        assert!(cache.get(c00).is_none(), "(0,0) must be evicted");
        assert!(cache.get(c01).is_some(), "(0,1) must remain");
        assert!(cache.get(c02).is_some(), "(0,2) must remain");
    }

    #[test]
    fn tile_cache_update_does_not_evict_prematurely() {
        let mut cache = TileCache::new(2);
        let c00 = TileCoord { tx: 0, ty: 0 };
        let c01 = TileCoord { tx: 0, ty: 1 };
        cache.insert(c00, solid_tile(1));
        cache.insert(c01, solid_tile(2));
        cache.insert(c00, solid_tile(9)); // update, no new entry
        assert!(cache.get(c00).is_some());
        assert!(cache.get(c01).is_some());
    }

    #[test]
    fn dirty_tracking_and_clean() {
        let mut cache = TileCache::new(16);
        let coord = TileCoord { tx: 1, ty: 1 };
        cache.insert(coord, TileData::transparent(TILE_SIZE));
        let dirty: Vec<TileCoord> = cache.dirty_coords().collect();
        assert_eq!(dirty, vec![coord]);
        cache.mark_clean(coord);
        assert_eq!(cache.dirty_coords().count(), 0);
    }

    #[test]
    fn remove_returns_tile_and_cleans_dirty() {
        let mut cache = TileCache::new(16);
        let coord = TileCoord { tx: 2, ty: 3 };
        cache.insert(coord, TileData::transparent(TILE_SIZE));
        let removed = cache.remove(coord);
        assert!(removed.is_some());
        assert!(cache.get(coord).is_none());
        assert_eq!(cache.dirty_coords().count(), 0);
    }
}
