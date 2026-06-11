// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, CropBounds, ExrCompression, Layer, LayerContent,
    PixelLayer, TileCache, TileCoord, TileData, LINEAR_SRGB, TILE_SIZE,
};
use crate::error::AifError;
use super::ldr::decode_ldr;
use super::exr::decode_exr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Exr,
}

pub(crate) fn detect_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.len() >= 4 && &bytes[0..4] == &[0x89, 0x50, 0x4E, 0x47] {
        return Some(ImageFormat::Png);
    }
    if bytes.len() >= 3 && &bytes[0..3] == &[0xFF, 0xD8, 0xFF] {
        return Some(ImageFormat::Jpeg);
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(ImageFormat::Webp);
    }
    if bytes.len() >= 4 && &bytes[0..4] == &[0x76, 0x2F, 0x31, 0x01] {
        return Some(ImageFormat::Exr);
    }
    None
}

/// Import a raster image from a raw byte buffer, converting standard formats
/// (PNG, JPEG, WebP, EXR) to a tiled f16 RGBA [`Layer`].
pub fn import_raster_image(bytes: &[u8], name: &str) -> Result<Layer, AifError> {
    let format = detect_format(bytes)
        .ok_or_else(|| AifError::ImportError("Unsupported or unrecognized image format".to_string()))?;

    let (width, height, pixels) = match format {
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp => decode_ldr(bytes)?,
        ImageFormat::Exr => decode_exr(bytes)?,
    };

    // Both decoders validate dimensions against crate::limits and return a
    // buffer of exactly width × height × 8 bytes; re-check here so the
    // unchecked indexing in the tile loop below is provably in bounds.
    let expected_len = crate::limits::checked_import_buffer_len(width, height)?;
    if pixels.len() != expected_len {
        return Err(AifError::ImportError(
            "decoded pixel buffer does not match image dimensions".to_string(),
        ));
    }

    let tile_size = TILE_SIZE;
    let cols = width.div_ceil(tile_size);
    let rows = height.div_ceil(tile_size);

    let mut tiles = TileCache::default();

    for ty in 0..rows {
        for tx in 0..cols {
            let mut tile_data = TileData::transparent(tile_size);

            for local_y in 0..tile_size {
                let global_y = ty * tile_size + local_y;
                if global_y >= height {
                    break;
                }
                for local_x in 0..tile_size {
                    let global_x = tx * tile_size + local_x;
                    if global_x >= width {
                        break;
                    }

                    let img_idx = (global_y as usize * width as usize + global_x as usize) * 8;
                    let tile_idx = (local_y as usize * tile_size as usize + local_x as usize) * 8;

                    tile_data.0[tile_idx..tile_idx + 8].copy_from_slice(&pixels[img_idx..img_idx + 8]);
                }
            }

            if !tile_data.is_fully_transparent() {
                tiles.insert(TileCoord { tx, ty }, tile_data);
            }
        }
    }

    Ok(Layer {
        id: uuid::Uuid::new_v4(),
        name: name.to_string(),
        visible: true,
        locked: false,
        opacity: 1.0,
        blend_mode: BlendMode::Normal,
        clipping_mask: false,
        mask: None,
        content: LayerContent::Pixel(PixelLayer {
            channel_layout: ChannelLayout::Rgba,
            bit_depth: BitDepth::F16,
            color_space: LINEAR_SRGB,
            compression: ExrCompression::Zip,
            canvas_offset_x: 0,
            canvas_offset_y: 0,
            crop_bounds: Some(CropBounds {
                x: 0,
                y: 0,
                width,
                height,
            }),
            tiles,
        }),
    })
}
