// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for iris-canvas — coordinate pipeline, no GPU required.

use iris_canvas::{CanvasViewport, TileKey};
use iris_pixel::{
    BlendMode, LayerContent, LayerTree, PixelLayer,
    BitDepth, ChannelLayout, ExrCompression, TileCache, TileCoord, TileData, TILE_SIZE,
    LINEAR_SRGB,
};
use uuid::Uuid;

fn make_tree() -> LayerTree {
    let mut tree = LayerTree::new(512, 512, 96.0, 96.0);
    let layer = iris_pixel::Layer {
        id: Uuid::new_v4(),
        name: "bg".into(),
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
            tiles: {
                let mut cache = TileCache::new(8);
                // Fill (0,0) with a distinguishable pattern (all bytes = 0x3C = f16 ~= 1.0)
                let data = TileData(vec![0x3Cu8; (TILE_SIZE as usize).pow(2) * 8].into_boxed_slice());
                cache.insert(TileCoord { tx: 0, ty: 0 }, data);
                cache
            },
        }),
    };
    tree.add_layer(None, 0, layer).expect("add layer");
    tree
}

#[test]
fn viewport_default_screen_center_maps_to_pan() {
    let vp = CanvasViewport::new(); // pan=(0,0), zoom=1, rotation=0
    let sw = 256u32;
    let sh = 256u32;
    // Screen center → pan (which is (0,0))
    let center_screen = kurbo::Vec2::new(128.0, 128.0);
    let doc = vp.screen_to_doc(center_screen, sw, sh);
    assert!(
        (doc.x - vp.pan.x).abs() < 1e-9 && (doc.y - vp.pan.y).abs() < 1e-9,
        "screen center should map to pan; got {doc:?}"
    );
}

#[test]
fn viewport_tile_selection_round_trip() {
    let sw = 256u32;
    let sh = 256u32;
    let vp = CanvasViewport::new(); // zoom=1, pan=(0,0), rotation=0

    // visible_doc_rect should cover the tile (0,0) doc bounds [0..256] × [0..256].
    let rect = vp.visible_doc_rect(sw, sh);
    // At zoom=1, pan=(0,0): visible rect is [-128..128] × [-128..128]
    // Tile (0,0) covers doc [0..256] × [0..256] — partially visible (0..128 visible).
    assert!(rect.x1 > 0.0 && rect.y1 > 0.0, "tile (0,0) should be partially visible");

    // Round-trip: screen → doc → screen for a sample of points.
    for &(sx, sy) in &[(0.0f64, 0.0), (128.0, 128.0), (200.0, 100.0)] {
        let screen = kurbo::Vec2::new(sx, sy);
        let doc = vp.screen_to_doc(screen, sw, sh);
        let back = vp.doc_to_screen(doc, sw, sh);
        assert!(
            (screen.x - back.x).abs() < 1e-9 && (screen.y - back.y).abs() < 1e-9,
            "round-trip failed for screen {screen:?}: got {back:?}"
        );
    }
}

#[test]
fn tile_key_equality_and_hash() {
    use std::collections::HashSet;
    let id = Uuid::new_v4();
    let c = TileCoord { tx: 3, ty: 7 };
    let k1 = TileKey::new(id, c);
    let k2 = TileKey::new(id, c);
    let k3 = TileKey::new(Uuid::new_v4(), c);
    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
    let mut set = HashSet::new();
    set.insert(k1);
    assert!(set.contains(&k2));
    assert!(!set.contains(&k3));
}

#[test]
fn layer_tree_has_one_visible_pixel_layer() {
    let tree = make_tree();
    let layers: Vec<_> = tree.iter_depth_first().collect();
    assert_eq!(layers.len(), 1);
    assert!(layers[0].visible);
    assert!(matches!(layers[0].content, LayerContent::Pixel(_)));
}
