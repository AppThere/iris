// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Generates and encodes the `iris/preview.png` part (§4.5).
//!
//! Phase 1 emits a 1×1 transparent sRGB PNG as a placeholder; a proper
//! compositor-rendered thumbnail is deferred to Phase 2.
//!
//! TODO(iris): SPEC.md §4.5 — generate a real 256×256 composite thumbnail.

use crate::error::AifError;

/// Write a minimal 1×1 transparent PNG and return the raw bytes.
///
/// Phase 1 placeholder — callers store the result as `iris/preview.png`.
pub(crate) fn write_preview_png() -> Result<Vec<u8>, AifError> {
    use png::{BitDepth, ColorType, Encoder};

    let mut buf = Vec::new();
    {
        let mut encoder = Encoder::new(&mut buf, 1, 1);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);

        let mut writer = encoder
            .write_header()
            .map_err(|e| AifError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        // Single fully-transparent pixel: [R=0, G=0, B=0, A=0]
        writer
            .write_image_data(&[0u8, 0, 0, 0])
            .map_err(|e| AifError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    }

    Ok(buf)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_valid_png_header() {
        let bytes = write_preview_png().expect("write preview");
        // PNG magic bytes: 137 80 78 71 13 10 26 10
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "must start with PNG magic");
    }

    #[test]
    fn non_empty_output() {
        let bytes = write_preview_png().expect("write preview");
        assert!(!bytes.is_empty());
    }
}
