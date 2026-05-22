// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use std::io::Cursor;
use exr::prelude::{f16, ReadChannels};
use exr::image::read::image::ReadLayers;
use crate::error::AifError;

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
            |res, _| ExrBuffer {
                width: res.width(),
                height: res.height(),
                pixels: vec![0u8; res.width() * res.height() * 8],
            },
            |buf: &mut ExrBuffer, pos, (r, g, b, a): (f16, f16, f16, f16)| {
                let base = (pos.y() * buf.width + pos.x()) * 8;
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
    if exr_buf.width == 0 || exr_buf.height == 0 {
        return Err(AifError::ImportError("Invalid or empty EXR dimensions".to_string()));
    }

    Ok((exr_buf.width as u32, exr_buf.height as u32, exr_buf.pixels))
}
