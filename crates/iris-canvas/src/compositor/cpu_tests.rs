// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the CPU composite path (`super::cpu`).
//!
//! These run without a GPU device: `composite_rgba8` is pure CPU.

use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerTree,
    PixelLayer, TileCache, TileCoord, TileData, LINEAR_SRGB, TILE_SIZE,
};

use super::cpu::{composite_rgba8, FrameMap};
use crate::viewport::CanvasViewport;

/// f16 1.0 little-endian.
const F16_ONE: [u8; 2] = [0x00, 0x3C];
/// f16 0.5 little-endian.
const F16_HALF: [u8; 2] = [0x00, 0x38];

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

/// Viewport centred on the document (matches `OpenDocument::new_blank`).
fn centered_viewport(doc_w: u32, doc_h: u32) -> CanvasViewport {
    let mut vp = CanvasViewport::new();
    vp.pan = kurbo::Vec2::new(doc_w as f64 / 2.0, doc_h as f64 / 2.0);
    vp
}

fn pixel(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = (y as usize * w as usize + x as usize) * 4;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

/// Regression: at DPI scale 2 every physical pixel of a painted region must be
/// covered. The old forward-mapping blit painted one physical pixel per
/// document pixel and left the rest white — a dithered checkerboard.
#[test]
fn high_dpi_scale_2_has_no_dither_holes() {
    let tree = red_tile_tree();
    let vp = centered_viewport(256, 256);
    // 256 logical × scale 2 = 512 physical; document exactly fills the frame.
    let buf = composite_rgba8(&tree, &vp, 512, 512, 2.0);

    // Sample well inside the document, including odd coordinates — the old
    // code left odd rows/columns unpainted.
    for (x, y) in [(101, 101), (100, 101), (101, 100), (255, 255), (256, 256), (33, 477)] {
        let p = pixel(&buf, 512, x, y);
        assert_eq!(p[3], 255, "alpha hole at ({x},{y}): {p:?}");
        assert!(p[0] > 200, "red missing at ({x},{y}): {p:?} — dither hole");
        assert!(p[1] < 30 && p[2] < 30, "expected red at ({x},{y}): {p:?}");
    }
}

/// Same property when zoomed in at scale 1: each document pixel covers a
/// 2×2 physical cluster and all four pixels must be painted.
#[test]
fn zoom_2_fills_every_cluster_pixel() {
    let tree = red_tile_tree();
    let mut vp = centered_viewport(256, 256);
    vp.zoom = 2.0;
    vp.pan = kurbo::Vec2::new(64.0, 64.0); // view doc [0,128)²
    let buf = composite_rgba8(&tree, &vp, 256, 256, 1.0);

    for (x, y) in [(0, 0), (1, 1), (128, 129), (254, 255)] {
        let p = pixel(&buf, 256, x, y);
        assert!(p[0] > 200 && p[3] == 255, "hole at ({x},{y}): {p:?}");
    }
}

/// Pixels outside the document stay transparent; inside-but-unpainted document
/// area is opaque white.
#[test]
fn background_white_inside_transparent_outside() {
    let tree = LayerTree::new(100, 100, 72.0, 72.0); // no layers painted
    let vp = centered_viewport(100, 100);
    // Logical frame 200×200 → document occupies the central 100×100.
    let buf = composite_rgba8(&tree, &vp, 200, 200, 1.0);

    let inside = pixel(&buf, 200, 100, 100);
    assert_eq!(inside, [255, 255, 255, 255], "inside doc must be white");
    let outside = pixel(&buf, 200, 10, 10);
    assert_eq!(outside[3], 0, "outside doc must be transparent: {outside:?}");
}

/// A 256×256 layer whose (0,0) tile is a solid opaque grey of the given f16
/// channel bytes, with the supplied blend mode.
fn gray_layer(chan: [u8; 2], mode: BlendMode) -> Layer {
    let ts = TILE_SIZE as usize;
    let mut bytes = vec![0u8; ts * ts * 8];
    for px in bytes.chunks_exact_mut(8) {
        px[0] = chan[0]; px[1] = chan[1]; // R
        px[2] = chan[0]; px[3] = chan[1]; // G
        px[4] = chan[0]; px[5] = chan[1]; // B
        px[6] = F16_ONE[0]; px[7] = F16_ONE[1]; // A = 1.0
    }
    let mut tiles = TileCache::new(4);
    tiles.insert(TileCoord { tx: 0, ty: 0 }, TileData::from_vec(bytes));
    Layer {
        id: uuid::Uuid::new_v4(),
        name: "gray".into(),
        visible: true,
        locked: false,
        opacity: 1.0,
        blend_mode: mode,
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
    }
}

/// Composite a 0.5-grey top layer over a 0.5-grey backdrop and return the centre
/// pixel's red channel for the given top-layer blend mode.
fn center_red_for_mode(mode: BlendMode) -> u8 {
    let mut tree = LayerTree::new(TILE_SIZE, TILE_SIZE, 72.0, 72.0);
    tree.add_layer(None, 0, gray_layer(F16_HALF, BlendMode::Normal)).expect("backdrop");
    tree.add_layer(None, 0, gray_layer(F16_HALF, mode)).expect("top"); // index 0 = top
    let vp = centered_viewport(256, 256);
    let buf = composite_rgba8(&tree, &vp, 256, 256, 1.0);
    pixel(&buf, 256, 128, 128)[0]
}

/// End-to-end wiring: a non-Normal blend mode must change the composite. Two
/// 0.5 greys multiply to 0.25 (linear), which is markedly darker than the
/// Normal result of 0.5. Locks in that `blit_tile` honours `layer.blend_mode`.
#[test]
fn multiply_blend_mode_darkens_composite() {
    let normal = center_red_for_mode(BlendMode::Normal);
    let multiply = center_red_for_mode(BlendMode::Multiply);
    let screen = center_red_for_mode(BlendMode::Screen);
    // sRGB(0.5) ≈ 188, sRGB(0.25) ≈ 137, sRGB(0.75) ≈ 224.
    assert!((180..=196).contains(&normal), "normal grey: {normal}");
    assert!((130..=145).contains(&multiply), "multiply grey: {multiply}");
    assert!(multiply < normal - 30, "multiply must darken: {multiply} vs {normal}");
    assert!(screen > normal + 20, "screen must lighten: {screen} vs {normal}");
}

/// `FrameMap` must agree exactly with `screen_to_doc` (the event-handler
/// mapping) for arbitrary zoom/pan/rotation.
#[test]
fn frame_map_matches_screen_to_doc() {
    let vp = CanvasViewport {
        pan: kurbo::Vec2::new(40.0, 60.0),
        zoom: 1.7,
        rotation: 0.3,
    };
    let (lw, lh) = (321u32, 199u32);
    let (sx_scale, sy_scale) = (1.5_f64, 1.5_f64);
    let map = FrameMap::new(&vp, lw, lh, sx_scale, sy_scale);
    for (px, py) in [(0.0, 0.0), (17.0, 3.0), (250.0, 180.0), (1.0, 197.0)] {
        let expected = vp.screen_to_doc(
            kurbo::Vec2::new((px + 0.5) / sx_scale, (py + 0.5) / sy_scale),
            lw,
            lh,
        );
        let got = map.doc_at(px, py);
        assert!(
            (got.x - expected.x).abs() < 1e-9 && (got.y - expected.y).abs() < 1e-9,
            "map mismatch at ({px},{py}): {got:?} vs {expected:?}"
        );
    }
}
