// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use std::io::Cursor;
use exr::prelude::{f16, ReadChannels};
use exr::image::read::image::ReadLayers;
use crate::error::AifError;
use crate::limits::checked_import_buffer_len;

pub(crate) struct ExrBuffer {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
}

pub(crate) fn decode_exr(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), AifError> {
    let cursor = Cursor::new(bytes);

    let image = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            // The create-closure cannot return an error, so dimensions that
            // fail validation get an empty sentinel buffer; the per-pixel
            // closure skips out-of-bounds writes and the sentinel becomes a
            // typed error after decoding.
            |res, _| {
                let buf_len = u32::try_from(res.width())
                    .ok()
                    .zip(u32::try_from(res.height()).ok())
                    .and_then(|(w, h)| checked_import_buffer_len(w, h).ok());
                ExrBuffer {
                    width: res.width(),
                    height: res.height(),
                    pixels: buf_len.map(|len| vec![0u8; len]).unwrap_or_default(),
                }
            },
            |buf: &mut ExrBuffer, pos, (r, g, b, a): (f16, f16, f16, f16)| {
                let base = (pos.y() * buf.width + pos.x()) * 8;
                if base + 8 > buf.pixels.len() {
                    return; // sentinel (rejected dimensions) or malformed position
                }
                let rb = r.to_bits().to_le_bytes();
                let gb = g.to_bits().to_le_bytes();
                let bb = b.to_bits().to_le_bytes();
                let ab = a.to_bits().to_le_bytes();
                buf.pixels[base] = rb[0];
                buf.pixels[base + 1] = rb[1];
                buf.pixels[base + 2] = gb[0];
                buf.pixels[base + 3] = gb[1];
                buf.pixels[base + 4] = bb[0];
                buf.pixels[base + 5] = bb[1];
                buf.pixels[base + 6] = ab[0];
                buf.pixels[base + 7] = ab[1];
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(cursor)
        .map_err(|e| AifError::ImportError(format!("Failed to decode EXR: {e}")))?;

    let exr_buf = image.layer_data.channel_data.pixels;
    let (width, height) = (
        u32::try_from(exr_buf.width).unwrap_or(u32::MAX),
        u32::try_from(exr_buf.height).unwrap_or(u32::MAX),
    );
    // Re-run the validation that selected the sentinel: a rejected or empty
    // buffer means the declared dimensions were out of range.
    let expected_len = checked_import_buffer_len(width, height)?;
    if exr_buf.pixels.len() != expected_len {
        return Err(AifError::ImportError(
            "EXR pixel buffer does not match declared dimensions".to_string(),
        ));
    }

    Ok((width, height, exr_buf.pixels))
}
