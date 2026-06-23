// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Document-level conversion from a parsed [`psd::Psd`] into an
//! [`iris_aif::AifDocument`]. Layer-tree construction lives in
//! [`crate::layers`].

use iris_aif::{parts, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BitDepth, LayerTree};
use psd::{ColorMode, Psd};
use uuid::Uuid;

use crate::error::PsdError;

// COMPAT(adobe): Photoshop stores resolution in a ResolutionInfo image-resource
// block (id 0x03ED) which the `psd` crate does not surface; the reader parses it
// (see crate::resources) and passes the result in as `dpi`.

/// Build an [`AifDocument`] from a parsed PSD and its `(dpi_x, dpi_y)`.
///
/// Layers are imported as RGBA f16 pixel layers in the Iris linear working
/// space, with group nesting and stacking order preserved.
pub(crate) fn psd_to_document(psd: &Psd, dpi: (f32, f32)) -> Result<AifDocument, PsdError> {
    require_supported_color_mode(psd.color_mode())?;

    let width = psd.width();
    let height = psd.height();

    let canvas = AifCanvas {
        mode: CanvasMode::Pixel,
        width_px: width,
        height_px: height,
        dpi_x: dpi.0,
        dpi_y: dpi.1,
        working_color_space: "linear-srgb".to_string(),
        bit_depth: BitDepth::F16,
    };

    let artboards = vec![AifArtboard {
        id: Uuid::new_v4(),
        name: "Canvas".to_string(),
        x_px: 0,
        y_px: 0,
        width_px: width,
        height_px: height,
    }];

    let mut tree = LayerTree::new(width, height, dpi.0, dpi.1);
    crate::layers::populate_tree(psd, &mut tree, width, height);

    Ok(AifDocument {
        canvas,
        artboards,
        layers: tree,
        format_version: (parts::AIF_MAJOR, parts::AIF_MINOR),
    })
}

/// Reject colour modes the Phase 1 importer cannot represent faithfully.
fn require_supported_color_mode(mode: ColorMode) -> Result<(), PsdError> {
    match mode {
        ColorMode::Rgb | ColorMode::Grayscale => Ok(()),
        // CMYK/Lab are deferred to Phase 4 (CMYK colour mode); Indexed,
        // Duotone, Bitmap, Multichannel are out of Phase 1 scope.
        other => Err(PsdError::UnsupportedColorMode(format!("{other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_and_grayscale_are_supported() {
        assert!(require_supported_color_mode(ColorMode::Rgb).is_ok());
        assert!(require_supported_color_mode(ColorMode::Grayscale).is_ok());
    }

    #[test]
    fn cmyk_is_rejected() {
        let err = require_supported_color_mode(ColorMode::Cmyk).unwrap_err();
        assert!(matches!(err, PsdError::UnsupportedColorMode(_)));
    }
}
