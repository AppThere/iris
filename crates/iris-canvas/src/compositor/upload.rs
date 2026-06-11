// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! CPU tile → GPU texture upload helpers.
//!
//! [`upload_tile`] converts a [`TileData`] (raw f16 RGBA bytes, straight alpha)
//! into a `wgpu::Texture` with format `Rgba16Float` on the given device/queue.
//! No CPU-side conversion is needed: `queue.write_texture` copies the bytes
//! directly; the f16 encoding is identical between TileData and Rgba16Float.

use iris_pixel::{TileData, TILE_SIZE};

/// Upload a [`TileData`] as an `Rgba16Float` GPU texture.
///
/// The tile texture has usage `TEXTURE_BINDING | COPY_DST` and is ready for
/// sampling immediately after this call returns.
pub(super) fn upload_tile(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tile_data: &TileData,
) -> wgpu::Texture {
    let texture = create_tile_texture(device);
    let size = wgpu::Extent3d {
        width: TILE_SIZE,
        height: TILE_SIZE,
        depth_or_array_layers: 1,
    };
    // bytes_per_row: TILE_SIZE pixels × 4 channels × 2 bytes/f16 = 2048
    let bytes_per_row = TILE_SIZE * 4 * 2;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        tile_data.bytes(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(TILE_SIZE),
        },
        size,
    );
    texture
}

/// Create a fully-transparent (all-zero) `Rgba16Float` tile texture.
///
/// Used when a tile coordinate is not present in the layer's cache —
/// absent tile = transparent = f16 zero for all channels.
pub(super) fn transparent_tile(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
    let texture = create_tile_texture(device);
    let size = wgpu::Extent3d {
        width: TILE_SIZE,
        height: TILE_SIZE,
        depth_or_array_layers: 1,
    };
    let bytes_per_row = TILE_SIZE * 4 * 2;
    let zeros = vec![0u8; (bytes_per_row * TILE_SIZE) as usize];
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &zeros,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(TILE_SIZE),
        },
        size,
    );
    texture
}

/// Allocate a 256×256 `Rgba16Float` texture for tile data.
fn create_tile_texture(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iris-canvas tile"),
        size: wgpu::Extent3d {
            width: TILE_SIZE,
            height: TILE_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
