// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Typed error enum for the PSD import adapter.

/// Errors produced while reading a Photoshop PSD/PSB document.
///
/// Every fatal condition is a distinct variant so callers can present a
/// precise message. Malformed input never panics out of this crate: a panic
/// from the underlying `psd` parser is caught and surfaced as
/// [`PsdError::Corrupt`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PsdError {
    /// The file does not begin with the `8BPS` PSD signature.
    #[error("not a PSD file (bad signature)")]
    BadSignature,

    /// The file is shorter than the 26-byte PSD file header.
    #[error("file too short to be a PSD ({0} bytes)")]
    Truncated(usize),

    /// The PSD version field is not `1`. Version `2` is PSB (large document),
    /// which is deferred to Phase 2.
    // COMPAT(adobe): the version field at offset 4 is `1` for `.psd` and `2`
    // for `.psb`. The `psd` crate only parses version 1.
    #[error("unsupported PSD version {0} (only version 1 is supported; 2 is PSB)")]
    UnsupportedVersion(u16),

    /// The document uses a colour mode the Phase 1 importer does not handle
    /// (e.g. CMYK, Lab, Indexed, Duotone, Multichannel, Bitmap). RGB and
    /// Grayscale are supported.
    #[error("unsupported PSD colour mode '{0}' (Phase 1 supports RGB and Grayscale)")]
    UnsupportedColorMode(String),

    /// The underlying `psd` parser rejected the file as malformed.
    #[error("malformed PSD: {0}")]
    Corrupt(String),

    /// The document exceeds PSD's 30,000-pixel-per-dimension limit (PSB, which
    /// allows up to 300,000, is not yet written).
    #[error("document {width}×{height} exceeds the PSD 30000px dimension limit")]
    DimensionsTooLarge {
        /// Canvas width in pixels.
        width: u32,
        /// Canvas height in pixels.
        height: u32,
    },

    /// I/O error while reading the file from disk or a reader.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_render() {
        assert_eq!(PsdError::BadSignature.to_string(), "not a PSD file (bad signature)");
        assert!(PsdError::UnsupportedVersion(2).to_string().contains("PSB"));
        assert!(PsdError::UnsupportedColorMode("Cmyk".into())
            .to_string()
            .contains("Cmyk"));
    }
}
