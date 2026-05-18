// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! LZ4-compressed pixel tile snapshot used by the undo/redo engine.

/// Error type for `iris-ops` crate operations.
#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    /// LZ4 decompression failed (corrupt or truncated snapshot bytes).
    #[error("lz4 decompression failed: {0}")]
    Decompression(String),
}

/// Opaque snapshot of a single pixel tile's raw f16 RGBA bytes, stored LZ4-compressed.
///
/// The uncompressed layout is 256 × 256 × 4 channels × 2 bytes (f16) = 524 288 bytes.
/// Storing it compressed reduces per-stroke undo memory overhead by roughly 60–80 % on
/// typical painted content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileSnapshot(pub(crate) Vec<u8>);

impl TileSnapshot {
    /// Compress raw f16 RGBA tile bytes into a snapshot.
    ///
    /// Uses [`lz4_flex::compress_prepend_size`], which prepends the original length
    /// and is infallible, so this method always succeeds.
    pub fn compress(raw: &[u8]) -> Self {
        Self(lz4_flex::compress_prepend_size(raw))
    }

    /// Decompress the snapshot back to raw f16 RGBA bytes.
    ///
    /// Returns [`OpsError::Decompression`] if the stored bytes are corrupt or truncated.
    pub fn decompress(&self) -> Result<Vec<u8>, OpsError> {
        lz4_flex::decompress_size_prepended(&self.0)
            .map_err(|e| OpsError::Decompression(e.to_string()))
    }

    /// Return the number of compressed bytes stored in this snapshot.
    pub fn compressed_len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_small() {
        let raw: Vec<u8> = (0u8..=255).cycle().take(512).collect();
        let snap = TileSnapshot::compress(&raw);
        let out = snap.decompress().expect("valid snapshot must decompress without error");
        assert_eq!(out, raw);
    }

    #[test]
    fn round_trip_full_tile() {
        // Simulate a full 256×256 f16 RGBA tile: 524 288 bytes.
        let raw: Vec<u8> = (0u8..=255).cycle().take(524_288).collect();
        let snap = TileSnapshot::compress(&raw);
        assert!(
            snap.compressed_len() < raw.len(),
            "lz4 must compress repeating data below original size"
        );
        let out = snap.decompress().expect("full-tile snapshot must decompress without error");
        assert_eq!(out, raw);
    }

    #[test]
    fn corrupt_bytes_return_error() {
        // A snapshot whose prepended size claims more bytes than are present.
        let corrupt = TileSnapshot(vec![0xFF, 0xFF, 0xFF, 0xFF, 0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(
            corrupt.decompress().is_err(),
            "corrupted snapshot bytes must return Err(OpsError::Decompression)"
        );
    }
}
