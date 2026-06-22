// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel tile primitives: [`TileData`] (raw f16 RGBA bytes) and [`TileCache`]
//! (capacity-bounded map with FIFO eviction and dirty tracking).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::tile_data::TileData;

/// Canonical tile size in pixels along each edge (256 × 256).
pub const TILE_SIZE: u32 = 256;

/// Re-export the canonical [`TileCoord`] type from `iris-ops` so callers
/// import from `iris_pixel` rather than `iris_ops` directly.
// TODO(iris): SPEC.md §3 — TileCoord is defined in iris-ops (audit §6.4).
// iris-pixel re-exports it here so callers use iris_pixel::TileCoord.
pub use iris_ops::TileCoord;

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
