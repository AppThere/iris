// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`TileData`] — raw f16 RGBA bytes for one tile, shared copy-on-write.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Monotonic source for [`TileData::revision`] values.
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

fn next_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, Ordering::Relaxed)
}

/// Raw f16 RGBA pixel data for one tile, shared copy-on-write.
///
/// Layout: `tile_size × tile_size × 4 channels × 2 bytes (f16)`.
/// For the default [`TILE_SIZE`][crate::TILE_SIZE] of 256 this is
/// 524 288 bytes per tile.
///
/// The buffer is behind an `Arc`: cloning a `TileData` (and therefore a
/// [`TileCache`][crate::TileCache] or a whole `LayerTree`) only bumps a
/// reference count, so per-event tree snapshots cost O(tiles) pointer copies
/// instead of O(pixels) memcpy. The first [`bytes_mut`][Self::bytes_mut] on a
/// shared tile deep-copies that one tile.
///
/// Every distinct pixel content carries a process-unique [`revision`][Self::revision]:
/// clones share it, mutation assigns a fresh one. Renderers use it as a cache
/// key for uploaded GPU textures.
#[derive(Debug, Clone)]
pub struct TileData {
    buf: Arc<Vec<u8>>,
    rev: u64,
}

impl PartialEq for TileData {
    fn eq(&self, other: &Self) -> bool {
        // Same revision ⇒ same content (clones); otherwise compare bytes.
        self.rev == other.rev || self.buf == other.buf
    }
}

impl Eq for TileData {}

impl TileData {
    /// Wrap an existing pixel buffer.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        TileData { buf: Arc::new(bytes), rev: next_revision() }
    }

    /// Allocate a fully-transparent tile (all channels = f16 `0.0`).
    ///
    /// `tile_size` is the edge length in pixels; use
    /// [`TILE_SIZE`][crate::TILE_SIZE] for the default 256-pixel tile.
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
        &self.buf
    }

    /// Mutable access to the raw pixel bytes.
    ///
    /// Copy-on-write: if this buffer is shared with another `TileData` clone
    /// (e.g. a render-thread snapshot), it is deep-copied once before the
    /// mutable borrow is handed out. Hoist this call out of per-pixel loops.
    /// Always assigns a fresh [`revision`][Self::revision].
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        self.rev = next_revision();
        Arc::make_mut(&mut self.buf).as_mut_slice()
    }

    /// Process-unique identifier for this pixel content. Clones share it;
    /// any [`bytes_mut`][Self::bytes_mut] call assigns a new one. Stable
    /// cache key for GPU tile uploads (no ABA risk, unlike pointer identity).
    pub fn revision(&self) -> u64 {
        self.rev
    }

    /// Returns `true` if every pixel's alpha channel is f16 `0.0`.
    ///
    /// f16 RGBA layout: each pixel is 8 bytes — R(2) G(2) B(2) A(2).
    /// Alpha occupies bytes 6–7 of each 8-byte group; f16 zero = `[0x00, 0x00]`.
    pub fn is_fully_transparent(&self) -> bool {
        self.buf.chunks_exact(8).all(|px| px[6] == 0 && px[7] == 0)
    }

    /// Return the number of bytes in this tile's raw pixel buffer.
    pub fn byte_len(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TILE_SIZE;

    fn solid_tile(value: u8) -> TileData {
        TileData::from_vec(vec![value; 64])
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
    fn revision_shared_by_clone_changed_by_write() {
        let mut a = solid_tile(1);
        let b = a.clone();
        assert_eq!(a.revision(), b.revision(), "clones share a revision");
        let before = a.revision();
        let _ = a.bytes_mut();
        assert_ne!(a.revision(), before, "bytes_mut must assign a new revision");
        assert_eq!(b.revision(), before, "other clone keeps the old revision");
    }

    #[test]
    fn distinct_tiles_have_distinct_revisions() {
        assert_ne!(solid_tile(0).revision(), solid_tile(0).revision());
    }

    #[test]
    fn equality_compares_content() {
        let a = solid_tile(3);
        let b = solid_tile(3); // different revision, same bytes
        assert_eq!(a, b);
        assert_ne!(a, solid_tile(4));
    }

    #[test]
    fn transparent_tile_is_fully_transparent_and_sized() {
        let tile = TileData::transparent(TILE_SIZE);
        assert!(tile.is_fully_transparent());
        assert_eq!(tile.byte_len(), 256 * 256 * 8);
    }
}
