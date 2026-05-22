// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use exr::prelude::f16;
use crate::error::AifError;

pub(crate) fn srgb_to_linear(u: f32) -> f32 {
    if u <= 0.04045 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn decode_ldr(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), AifError> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| AifError::ImportError(format!("Failed to decode image: {e}")))?;

    let width = img.width();
    let height = img.height();
    let rgba = img.to_rgba8();

    let mut pixels = vec![0u8; width as usize * height as usize * 8];

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let r_srgb = pixel.0[0] as f32 / 255.0;
        let g_srgb = pixel.0[1] as f32 / 255.0;
        let b_srgb = pixel.0[2] as f32 / 255.0;
        let a_linear = pixel.0[3] as f32 / 255.0;

        let r_linear = srgb_to_linear(r_srgb);
        let g_linear = srgb_to_linear(g_srgb);
        let b_linear = srgb_to_linear(b_srgb);

        let r_f16 = f16::from_f32(r_linear);
        let g_f16 = f16::from_f32(g_linear);
        let b_f16 = f16::from_f32(b_linear);
        let a_f16 = f16::from_f32(a_linear);

        let base = (y as usize * width as usize + x as usize) * 8;
        let rb = r_f16.to_bits().to_le_bytes();
        let gb = g_f16.to_bits().to_le_bytes();
        let bb = b_f16.to_bits().to_le_bytes();
        let ab = a_f16.to_bits().to_le_bytes();

        pixels[base] = rb[0];
        pixels[base + 1] = rb[1];
        pixels[base + 2] = gb[0];
        pixels[base + 3] = gb[1];
        pixels[base + 4] = bb[0];
        pixels[base + 5] = bb[1];
        pixels[base + 6] = ab[0];
        pixels[base + 7] = ab[1];
    }

    Ok((width, height, pixels))
}
