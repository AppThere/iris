// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`PixelLayer`] — the raster-data payload for a [`crate::layer::LayerContent::Pixel`] layer.

use crate::color_space::ColorSpaceId;
use crate::tile::{TileCache, TILE_SIZE};

/// Raster pixel layer backed by a [`TileCache`].
///
/// All spatial coordinates are in document pixels. The layer may be smaller
/// than the canvas (offset + crop) or extend beyond it.
#[derive(Debug, Clone)]
pub struct PixelLayer {
    /// Arrangement of colour channels in tile memory.
    pub channel_layout: ChannelLayout,
    /// Numeric precision per channel.
    pub bit_depth: BitDepth,
    /// Colour space of the pixel data (SPEC.md §4.9).
    pub color_space: ColorSpaceId,
    /// Internal OpenEXR compression codec used when flushing tiles to disk.
    pub compression: ExrCompression,
    /// Horizontal offset of this layer's top-left corner relative to the canvas origin.
    pub canvas_offset_x: i32,
    /// Vertical offset of this layer's top-left corner relative to the canvas origin.
    pub canvas_offset_y: i32,
    /// Optional crop rectangle within the layer's tile data.
    pub crop_bounds: Option<CropBounds>,
    /// Tile data cache — the authoritative pixel store for this layer.
    pub tiles: TileCache,
}

impl PixelLayer {
    /// Number of tiles needed to cover `pixel_count` pixels at the default tile size.
    pub fn tiles_needed(pixel_count: u32) -> u32 {
        pixel_count.div_ceil(TILE_SIZE)
    }

    /// Number of tiles needed to cover this layer's `crop_bounds`, or `(0, 0)`
    /// if no crop is set.
    pub fn cropped_tile_dims(&self) -> (u32, u32) {
        match &self.crop_bounds {
            Some(b) => (Self::tiles_needed(b.width), Self::tiles_needed(b.height)),
            None => (0, 0),
        }
    }
}

/// Arrangement of colour channels stored in a [`PixelLayer`]'s tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelLayout {
    /// Red, green, blue, alpha (the Phase 1 working layout).
    Rgba,
    /// Red, green, blue (no alpha channel).
    Rgb,
    /// Luminance plus alpha.
    La,
    /// Luminance only (greyscale, no alpha).
    L,
    // TODO(iris): SPEC.md §4.9 — Phase 4: CMYK requires appthere-color CMYK extension (ADR-003).
    /// Cyan, magenta, yellow, key (black) — Phase 4 only.
    Cmyk,
}

/// Numeric precision per channel in a [`PixelLayer`]'s tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitDepth {
    /// 8-bit unsigned integer (0–255).
    U8,
    /// 16-bit unsigned integer (0–65535).
    U16,
    /// 16-bit float (IEEE 754 half precision) — Phase 1 compositor working depth.
    F16,
    /// 32-bit float (IEEE 754 single precision).
    F32,
}

/// Internal OpenEXR compression codec applied when tiles are flushed to disk.
///
/// These are EXR-internal codecs, distinct from the OPC/ZIP container
/// compression described in ADR-002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExrCompression {
    /// ZIP compression, multi-scanline blocks.
    Zip,
    /// ZIP compression, single-scanline blocks.
    Zips,
    /// Wavelet (PIZ) compression — good ratio on natural images.
    Piz,
    /// DWA compression, larger block size.
    Dwab,
    /// DWA compression, single-scanline blocks.
    Dwaa,
}

/// Crop rectangle within a [`PixelLayer`]'s tile data.
///
/// Coordinates are in layer-local pixels (i.e. relative to the layer's own
/// origin, not the canvas origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropBounds {
    /// Left edge of the crop in layer pixels.
    pub x: i32,
    /// Top edge of the crop in layer pixels.
    pub y: i32,
    /// Width of the crop in pixels.
    pub width: u32,
    /// Height of the crop in pixels.
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_needed_exact_multiple() {
        assert_eq!(PixelLayer::tiles_needed(256), 1);
        assert_eq!(PixelLayer::tiles_needed(512), 2);
        assert_eq!(PixelLayer::tiles_needed(0), 0);
    }

    #[test]
    fn tiles_needed_partial_tile() {
        assert_eq!(PixelLayer::tiles_needed(1), 1);
        assert_eq!(PixelLayer::tiles_needed(257), 2);
        assert_eq!(PixelLayer::tiles_needed(1920), 8); // ceil(1920/256)
    }
}
