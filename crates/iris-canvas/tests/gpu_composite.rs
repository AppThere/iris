// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Headless GPU compositor tests.
//!
//! Requires any wgpu adapter (a software rasteriser such as lavapipe is
//! sufficient — CI installs `mesa-vulkan-drivers`). When no adapter exists
//! the tests skip with a note rather than failing, so plain environments
//! still pass.

use std::io::Write as _;

use iris_canvas::{composite_rgba8_cpu, CanvasViewport, Compositor};
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerTree,
    PixelLayer, TileCache, TileCoord, TileData, LINEAR_SRGB, TILE_SIZE,
};

/// f16 1.0 little-endian.
const F16_ONE: [u8; 2] = [0x00, 0x3C];

fn gpu_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    pollster::block_on(async {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .ok()?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .ok()?;
        Some((device, queue))
    })
}

/// A 256×256 document with one layer whose (0,0) tile is solid opaque red.
fn red_tile_tree() -> LayerTree {
    let ts = TILE_SIZE as usize;
    let mut bytes = vec![0u8; ts * ts * 8];
    for px in bytes.chunks_exact_mut(8) {
        px[0] = F16_ONE[0]; // R = 1.0
        px[1] = F16_ONE[1];
        px[6] = F16_ONE[0]; // A = 1.0
        px[7] = F16_ONE[1];
    }
    let mut tiles = TileCache::new(4);
    tiles.insert(TileCoord { tx: 0, ty: 0 }, TileData::from_vec(bytes));
    let layer = Layer {
        id: uuid::Uuid::nil(),
        name: "red".into(),
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
            crop_bounds: None,
            tiles,
        }),
    };
    let mut tree = LayerTree::new(TILE_SIZE, TILE_SIZE, 72.0, 72.0);
    tree.add_layer(None, 0, layer).expect("add layer");
    tree
}

/// Read an `Rgba8Unorm` texture back into a byte buffer.
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (texture.width(), texture.height());
    let bytes_per_row = w * 4; // tests use widths that are already 256-aligned
    assert_eq!(bytes_per_row % 256, 0, "test sizes must be aligned for readback");
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback buffer"));
    device.poll(wgpu::PollType::Wait).expect("device poll");
    let data = slice.get_mapped_range().to_vec();
    data
}

fn pixel(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = (y as usize * w as usize + x as usize) * 4;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

/// The GPU frame path must agree with the CPU reference implementation away
/// from quad edges (filtering differs at boundaries by design).
#[test]
fn gpu_frame_matches_cpu_reference() {
    let Some((device, queue)) = gpu_device() else {
        let _ = writeln!(std::io::stderr(), "no wgpu adapter — skipping GPU test");
        return;
    };
    let tree = red_tile_tree();
    let mut viewport = CanvasViewport::new();
    viewport.pan = kurbo::Vec2::new(128.0, 128.0);

    // 256 logical × scale 2 = 512 physical (512×4 bytes/row = aligned).
    let compositor = Compositor::new();
    let texture = compositor
        .composite_frame(&tree, &viewport, 512, 512, 2.0, &device, &queue)
        .expect("gpu composite");
    let gpu = read_back(&device, &queue, &texture);
    let cpu = composite_rgba8_cpu(&tree, &viewport, 512, 512, 2.0);

    // Interior samples: red tile fills the document, document fills the frame.
    for (x, y) in [(8, 8), (101, 100), (255, 256), (300, 477), (503, 503)] {
        let g = pixel(&gpu, 512, x, y);
        let c = pixel(&cpu, 512, x, y);
        for ch in 0..4 {
            assert!(
                (g[ch] as i16 - c[ch] as i16).abs() <= 3,
                "GPU/CPU mismatch at ({x},{y}) ch{ch}: gpu={g:?} cpu={c:?}"
            );
        }
    }
}

/// Background semantics must match the CPU path: white inside the document,
/// transparent outside.
#[test]
fn gpu_frame_background_white_inside_transparent_outside() {
    let Some((device, queue)) = gpu_device() else {
        let _ = writeln!(std::io::stderr(), "no wgpu adapter — skipping GPU test");
        return;
    };
    // 128×128 document centred in a 256-logical frame at scale 1.
    let tree = LayerTree::new(128, 128, 72.0, 72.0);
    let mut viewport = CanvasViewport::new();
    viewport.pan = kurbo::Vec2::new(64.0, 64.0);

    let compositor = Compositor::new();
    let texture = compositor
        .composite_frame(&tree, &viewport, 256, 256, 1.0, &device, &queue)
        .expect("gpu composite");
    let gpu = read_back(&device, &queue, &texture);

    let inside = pixel(&gpu, 256, 128, 128);
    assert_eq!(inside, [255, 255, 255, 255], "inside doc must be white: {inside:?}");
    let outside = pixel(&gpu, 256, 10, 10);
    assert_eq!(outside[3], 0, "outside doc must be transparent: {outside:?}");
}

/// Painting a tile (new revision) must invalidate the cached GPU upload —
/// the second frame must show the new pixels, not the cached ones.
#[test]
fn gpu_frame_tile_revision_invalidation() {
    let Some((device, queue)) = gpu_device() else {
        let _ = writeln!(std::io::stderr(), "no wgpu adapter — skipping GPU test");
        return;
    };
    let mut tree = red_tile_tree();
    let mut viewport = CanvasViewport::new();
    viewport.pan = kurbo::Vec2::new(128.0, 128.0);

    let compositor = Compositor::new();
    let first = compositor
        .composite_frame(&tree, &viewport, 256, 256, 1.0, &device, &queue)
        .expect("first frame");
    let before = read_back(&device, &queue, &first);
    assert!(pixel(&before, 256, 128, 128)[0] > 200, "first frame is red");

    // Repaint the tile green through the CoW write path.
    let ids: Vec<_> = tree.root_layer_ids().to_vec();
    let layer = tree.get_mut(ids[0]).expect("layer");
    if let LayerContent::Pixel(ref mut px) = layer.content {
        let tile = px.tiles.get_mut(TileCoord { tx: 0, ty: 0 }).expect("tile");
        for p in tile.bytes_mut().chunks_exact_mut(8) {
            p[0] = 0; // R = 0
            p[1] = 0;
            p[2] = F16_ONE[0]; // G = 1.0
            p[3] = F16_ONE[1];
        }
    }

    let second = compositor
        .composite_frame(&tree, &viewport, 256, 256, 1.0, &device, &queue)
        .expect("second frame");
    let after = read_back(&device, &queue, &second);
    let p = pixel(&after, 256, 128, 128);
    assert!(p[1] > 200 && p[0] < 30, "repainted tile must be green, got {p:?}");
}
