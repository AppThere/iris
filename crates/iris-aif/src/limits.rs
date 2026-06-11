// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Hard limits on dimensions read from untrusted input.
//!
//! Image decoders and XML parsers must never allocate or iterate based on a
//! file-supplied dimension without validating it here first. A crafted file
//! claiming enormous dimensions must fail with a typed error instead of
//! exhausting memory, overflowing arithmetic, or spinning in a tile loop.

use crate::error::AifError;

/// Maximum width or height (pixels) accepted from an imported raster image.
pub const MAX_IMPORT_DIMENSION: u32 = 65_536;

/// Maximum total pixel count accepted from an imported raster image.
/// 2^28 pixels × 8 bytes (f16 RGBA) caps the decode buffer at 2 GiB.
pub const MAX_IMPORT_PIXELS: u64 = 1 << 28;

/// Maximum canvas / artboard / crop-bounds dimension (pixels) accepted from
/// document XML. Matches the order of magnitude of PSB's 300 000 px ceiling
/// and bounds the per-layer tile loop to ~one million iterations.
pub const MAX_CANVAS_DIMENSION: u32 = 262_144;

/// Validate imported image dimensions and return the exact f16-RGBA buffer
/// length (`w × h × 8`) they require.
pub(crate) fn checked_import_buffer_len(width: u32, height: u32) -> Result<usize, AifError> {
    if width == 0 || height == 0 {
        return Err(AifError::ImportError("image has zero width or height".into()));
    }
    if width > MAX_IMPORT_DIMENSION || height > MAX_IMPORT_DIMENSION {
        return Err(AifError::ImportError(format!(
            "image dimensions {width}×{height} exceed the maximum of \
             {MAX_IMPORT_DIMENSION} px per side"
        )));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_IMPORT_PIXELS {
        return Err(AifError::ImportError(format!(
            "image has {pixels} pixels, exceeding the maximum of {MAX_IMPORT_PIXELS}"
        )));
    }
    // In-range by construction: MAX_IMPORT_PIXELS × 8 fits comfortably in u64,
    // and usize is at least 32 bits (2 GiB cap) on all supported targets.
    usize::try_from(pixels * 8).map_err(|_| {
        AifError::ImportError("image buffer size exceeds addressable memory".into())
    })
}

/// Validate a dimension parsed from document XML (canvas, artboard, or
/// crop bounds). `what` names the attribute for the error message.
pub(crate) fn validate_xml_dimension(
    value: u32,
    what: &str,
    part: &str,
) -> Result<(), AifError> {
    if value > MAX_CANVAS_DIMENSION {
        return Err(AifError::XmlParse {
            part: part.into(),
            message: format!(
                "{what} = {value} exceeds the maximum supported dimension \
                 of {MAX_CANVAS_DIMENSION} px"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_dimensions() {
        assert_eq!(checked_import_buffer_len(10, 10).map_err(|_| ()), Ok(800));
    }

    #[test]
    fn rejects_zero_dimension() {
        assert!(checked_import_buffer_len(0, 10).is_err());
        assert!(checked_import_buffer_len(10, 0).is_err());
    }

    #[test]
    fn rejects_oversized_side() {
        assert!(checked_import_buffer_len(MAX_IMPORT_DIMENSION + 1, 1).is_err());
        assert!(checked_import_buffer_len(1, MAX_IMPORT_DIMENSION + 1).is_err());
    }

    #[test]
    fn rejects_excessive_pixel_count() {
        // Each side is legal but the area is not.
        assert!(checked_import_buffer_len(65_536, 65_536).is_err());
    }

    #[test]
    fn xml_dimension_boundary() {
        assert!(validate_xml_dimension(MAX_CANVAS_DIMENSION, "widthPx", "p").is_ok());
        assert!(validate_xml_dimension(MAX_CANVAS_DIMENSION + 1, "widthPx", "p").is_err());
    }
}
