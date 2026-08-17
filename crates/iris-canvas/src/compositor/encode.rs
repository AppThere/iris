// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Numeric conversions and texture upload for the CPU composite path.
//!
//! The accumulation buffer is premultiplied linear light; the GPU expects
//! straight-alpha sRGB. Everything that crosses that boundary lives here.

use crate::compositor::CompositorError;

/// Convert the accumulation buffer to sRGB u8 and upload it as a texture.
pub(super) fn upload_acc(
    acc: &[f32],
    pixel_count: usize,
    width_px: u32,
    height_px: u32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<wgpu::Texture, CompositorError> {
    // Convert premultiplied linear → straight-alpha sRGB u8 for upload.
    let mut u8_data = Vec::with_capacity(pixel_count * 4);
    for i in 0..pixel_count {
        let base = i * 4;
        let a = acc[base + 3].clamp(0.0, 1.0);
        let (r, g, b) = if a > f32::EPSILON {
            (acc[base] / a, acc[base + 1] / a, acc[base + 2] / a)
        } else {
            (0.0, 0.0, 0.0)
        };
        u8_data.push(to_srgb_u8(r));
        u8_data.push(to_srgb_u8(g));
        u8_data.push(to_srgb_u8(b));
        u8_data.push((a * 255.0 + 0.5) as u8);
    }

    // COMPAT(blitz): Rgba8Unorm + COPY_SRC + COPY_DST is required by
    // vello::Renderer::register_texture(), which copies the texture into
    // Vello's image atlas via a wgpu copy operation (needs COPY_SRC).
    // COPY_DST is required by queue.write_texture() for the initial upload.
    // TEXTURE_BINDING and STORAGE_BINDING are required by anyrender_vello's blit pass
    // which samples the texture in a shader (vello::Renderer::register_texture path).
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iris-canvas-composite"),
        size: wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &u8_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width_px * 4),
            rows_per_image: Some(height_px),
        },
        wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
    );
    Ok(texture)
}

/// Convert IEEE 754 half-precision bits to f32.
/// Subnormal f16 values (exp=0, mant≠0) are treated as zero — adequate for pixels.
pub(super) fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp  = ((bits as u32) & 0x7C00) >> 10;
    let mant = (bits as u32) & 0x03FF;
    f32::from_bits(match exp {
        0  => sign,                                     // ±zero or subnormal → zero
        31 => sign | 0x7F80_0000 | (mant << 13),       // ±inf / NaN pass-through
        e  => sign | ((e + 112) << 23) | (mant << 13), // normal: rebias 15→127
    })
}

/// Encode a linear-light [0,1] value to sRGB gamma and clamp to u8.
///
/// Inverse of [`crate::vector::srgb_to_linear`].
pub(crate) fn to_srgb_u8(linear: f32) -> u8 {
    let s = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.clamp(0.0, 1.0).powf(1.0 / 2.4) - 0.055
    };
    (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_round_numbers_convert() {
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x3C00), 1.0);
        assert_eq!(f16_to_f32(0x4000), 2.0);
        assert_eq!(f16_to_f32(0xBC00), -1.0);
    }

    #[test]
    fn srgb_encoding_hits_known_endpoints() {
        assert_eq!(to_srgb_u8(0.0), 0);
        assert_eq!(to_srgb_u8(1.0), 255);
        // Linear 0.5 encodes to sRGB ~0.7354 → 188.
        assert_eq!(to_srgb_u8(0.5), 188);
    }

    #[test]
    fn srgb_encoding_clamps_out_of_range() {
        assert_eq!(to_srgb_u8(-1.0), 0);
        assert_eq!(to_srgb_u8(4.0), 255);
    }
}
