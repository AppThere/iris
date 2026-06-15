// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Conversion from a parsed [`psd::Psd`] into an [`iris_aif::AifDocument`].

use std::panic::{catch_unwind, AssertUnwindSafe};

use iris_aif::{layer_from_rgba8, parts, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BitDepth, Layer, LayerTree};
use psd::{ColorMode, Psd, PsdLayer};
use uuid::Uuid;

use crate::error::PsdError;

// COMPAT(adobe): Photoshop stores resolution in an image-resource block
// (id 0x03ED). The `psd` crate does not surface it as a typed value, so until
// that block is parsed we assume Photoshop's 72 dpi screen default.
// TODO(iris): SPEC.md §4.4 — read ResolutionInfo (0x03ED) for true dpi.
const DEFAULT_DPI: f32 = 72.0;

/// Build an [`AifDocument`] from a parsed PSD.
///
/// Layers are imported as RGBA f16 pixel layers in the Iris linear working
/// space, ordered to match Photoshop's stacking (top of the panel first).
pub(crate) fn psd_to_document(psd: &Psd) -> Result<AifDocument, PsdError> {
    require_supported_color_mode(psd.color_mode())?;

    let width = psd.width();
    let height = psd.height();

    let canvas = AifCanvas {
        mode: CanvasMode::Pixel,
        width_px: width,
        height_px: height,
        dpi_x: DEFAULT_DPI,
        dpi_y: DEFAULT_DPI,
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

    let mut tree = LayerTree::new(width, height, DEFAULT_DPI, DEFAULT_DPI);
    populate_layers(psd, &mut tree, width, height);

    Ok(AifDocument {
        canvas,
        artboards,
        layers: tree,
        format_version: (parts::AIF_MAJOR, parts::AIF_MINOR),
    })
}

/// Fill `tree` with one Iris layer per PSD layer.
///
/// `psd.layers()` is ordered bottom-to-top; inserting each at root position 0
/// reproduces Photoshop's top-first panel order. Layers that fail to
/// rasterise are skipped with a warning rather than aborting the whole import.
fn populate_layers(psd: &Psd, tree: &mut LayerTree, width: u32, height: u32) {
    // COMPAT(adobe): a PSD that was never given an explicit layer has an empty
    // layer-and-mask section; its pixels live only in the merged image. Fall
    // back to the composite as a single "Background" layer.
    if psd.layers().is_empty() {
        if let Some(layer) = composite_layer(psd, width, height) {
            let _ = tree.add_layer(None, 0, layer);
        }
        return;
    }

    for psd_layer in psd.layers() {
        if let Some(layer) = convert_layer(psd_layer, width, height) {
            // Insert at the top each time so the last PSD layer (topmost) ends
            // up at root index 0.
            let _ = tree.add_layer(None, 0, layer);
        }
    }
}

/// Convert a single PSD layer into an Iris [`Layer`], or `None` if it cannot
/// be rasterised.
fn convert_layer(psd_layer: &PsdLayer, width: u32, height: u32) -> Option<Layer> {
    // COMPAT(adobe): the `psd` crate unwraps internally when a layer is missing
    // an expected channel (e.g. divider/section layers). Isolate the panic so a
    // single odd layer cannot crash the host application.
    let rgba = match catch_unwind(AssertUnwindSafe(|| psd_layer.rgba())) {
        Ok(rgba) => rgba,
        Err(_) => {
            tracing::warn!(name = psd_layer.name(), "skipping PSD layer that failed to rasterise");
            return None;
        }
    };

    // TODO(iris): SPEC.md §4.6 — crop to layer_left/top + width/height and set
    // canvas_offset to reduce memory; `rgba()` currently returns a full-canvas
    // buffer per layer.
    let mut layer = layer_from_rgba8(width, height, &rgba, psd_layer.name());
    layer.visible = psd_layer.visible();
    layer.opacity = psd_layer.opacity() as f32 / 255.0;
    layer.blend_mode = crate::blend::map_blend_mode(&format!("{:?}", psd_layer.blend_mode()));
    layer.clipping_mask = psd_layer.is_clipping_mask();
    Some(layer)
}

/// Build a single "Background" layer from the merged composite image.
fn composite_layer(psd: &Psd, width: u32, height: u32) -> Option<Layer> {
    let rgba = catch_unwind(AssertUnwindSafe(|| psd.rgba())).ok()?;
    Some(layer_from_rgba8(width, height, &rgba, "Background"))
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
