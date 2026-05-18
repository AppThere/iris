// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Public document model types for the Artisan Interchange Format.
//!
//! These types are the currency passed between [`crate::reader::AifReader`]
//! and [`crate::writer::AifWriter`]. They mirror the XML schema in
//! SPEC.md §4.4 closely but use idiomatic Rust field names.

use uuid::Uuid;

/// A fully-decoded AIF document ready for use by the compositor.
///
/// Produced by `AifReader` on open and consumed by `AifWriter` on save.
#[derive(Debug)]
pub struct AifDocument {
    /// Canvas geometry and working colour space.
    pub canvas: AifCanvas,
    /// Artboard layout. Pixel-mode documents have exactly one artboard.
    pub artboards: Vec<AifArtboard>,
    /// The document's layer tree with all pixel tile data.
    pub layers: iris_pixel::LayerTree,
    /// The `formatVersion` as read from `document.xml`, e.g. `(1, 0)`.
    /// Writers always emit [`crate::parts::AIF_MAJOR`]`.`[`crate::parts::AIF_MINOR`].
    pub format_version: (u32, u32),
}

/// Canvas geometry and working-space description (§4.4 `<iris:Canvas>`).
#[derive(Debug, Clone)]
pub struct AifCanvas {
    /// Pixel vs. vector vs. mixed editing mode.
    pub mode: CanvasMode,
    /// Canvas width in pixels at `dpi_x`.
    pub width_px: u32,
    /// Canvas height in pixels at `dpi_y`.
    pub height_px: u32,
    /// Horizontal resolution in dots per inch.
    pub dpi_x: f32,
    /// Vertical resolution in dots per inch.
    pub dpi_y: f32,
    /// Working colour space identifier string (§4.9).
    pub working_color_space: String,
    /// Per-channel numeric precision for pixel layers.
    pub bit_depth: iris_pixel::BitDepth,
}

/// Document editing mode (§4.4 `<iris:Canvas mode="…">`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanvasMode {
    /// Raster pixel editing; exactly one artboard matching canvas bounds.
    Pixel,
    /// Vector path editing; one or more artboards.
    Vector,
    /// Unified pixel + vector editing; one or more artboards.
    Mixed,
}

/// A named rectangular clip scope in the document (§4.4 `<iris:Artboard>`).
#[derive(Debug, Clone)]
pub struct AifArtboard {
    /// Stable UUID assigned at document creation.
    pub id: Uuid,
    /// Display name shown in the artboards panel.
    pub name: String,
    /// X origin in canvas pixels (may be negative in vector mode).
    pub x_px: i32,
    /// Y origin in canvas pixels.
    pub y_px: i32,
    /// Artboard width in pixels.
    pub width_px: u32,
    /// Artboard height in pixels.
    pub height_px: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_mode_eq() {
        assert_eq!(CanvasMode::Pixel, CanvasMode::Pixel);
        assert_ne!(CanvasMode::Pixel, CanvasMode::Vector);
    }

    #[test]
    fn artboard_fields_accessible() {
        let ab = AifArtboard {
            id: Uuid::nil(),
            name: "Canvas".into(),
            x_px: 0,
            y_px: 0,
            width_px: 800,
            height_px: 600,
        };
        assert_eq!(ab.width_px, 800);
    }
}
