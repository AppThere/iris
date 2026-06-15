// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Write → read round-trip tests for `iris-psd`.
//!
//! Documents are built directly with iris-aif/iris-pixel, serialised with
//! [`PsdWriter`], and read back with [`PsdReader`]. This validates that the
//! hand-rolled PSD writer produces files our own (format-faithful) reader
//! accepts, and that structure and pixels survive the trip.

use iris_aif::{layer_from_rgba8, layer_to_rgba8, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BitDepth, BlendMode, Layer, LayerContent, LayerTree};
use iris_psd::{PsdError, PsdReader, PsdWriter};

/// 2×2 RGBA: red, green, blue, yellow — all opaque.
fn swatch() -> Vec<u8> {
    vec![
        255, 0, 0, 255, // (0,0) red
        0, 255, 0, 255, // (1,0) green
        0, 0, 255, 255, // (0,1) blue
        255, 255, 0, 255, // (1,1) yellow
    ]
}

fn document(tree: LayerTree) -> AifDocument {
    let (w, h) = (tree.canvas_width, tree.canvas_height);
    AifDocument {
        canvas: AifCanvas {
            mode: CanvasMode::Pixel,
            width_px: w,
            height_px: h,
            dpi_x: 72.0,
            dpi_y: 72.0,
            working_color_space: "linear-srgb".to_string(),
            bit_depth: BitDepth::F16,
        },
        artboards: vec![AifArtboard {
            id: uuid::Uuid::new_v4(),
            name: "Canvas".to_string(),
            x_px: 0,
            y_px: 0,
            width_px: w,
            height_px: h,
        }],
        layers: tree,
        format_version: (1, 0),
    }
}

fn group_node(name: &str) -> Layer {
    Layer {
        id: uuid::Uuid::new_v4(),
        name: name.to_string(),
        visible: true,
        locked: false,
        opacity: 1.0,
        blend_mode: BlendMode::Normal,
        clipping_mask: false,
        mask: None,
        content: LayerContent::Group { children: vec![] },
    }
}

#[test]
fn round_trips_single_pixel_layer() {
    let mut tree = LayerTree::new(2, 2, 72.0, 72.0);
    let mut layer = layer_from_rgba8(2, 2, &swatch(), "Layer 1");
    layer.blend_mode = BlendMode::Multiply;
    layer.opacity = 0.5;
    tree.add_layer(None, 0, layer).expect("add layer");

    let bytes = PsdWriter::to_bytes(&document(tree)).expect("write PSD");
    let back = PsdReader::from_bytes(&bytes).expect("read PSD back");

    assert_eq!(back.canvas.width_px, 2);
    assert_eq!(back.canvas.height_px, 2);
    let root = back.layers.root_layer_ids();
    assert_eq!(root.len(), 1);

    let layer = back.layers.get(root[0]).expect("layer present");
    assert_eq!(layer.name, "Layer 1");
    assert_eq!(layer.blend_mode, BlendMode::Multiply);
    assert!((layer.opacity - 0.5).abs() < 0.01, "opacity {} preserved", layer.opacity);

    // The top-left pixel must come back red and opaque.
    let px = layer_to_rgba8(layer).expect("read pixels");
    assert!(px.rgba[0] > 200, "R = {}", px.rgba[0]);
    assert!(px.rgba[1] < 50, "G = {}", px.rgba[1]);
    assert!(px.rgba[2] < 50, "B = {}", px.rgba[2]);
    assert!(px.rgba[3] > 200, "A = {}", px.rgba[3]);
}

#[test]
fn round_trips_group_hierarchy() {
    let mut tree = LayerTree::new(2, 2, 72.0, 72.0);
    let gid = tree.add_layer(None, 0, group_node("Group A")).expect("add group");
    let child = layer_from_rgba8(2, 2, &swatch(), "Layer 1");
    tree.add_layer(Some(gid), 0, child).expect("add child");

    let bytes = PsdWriter::to_bytes(&document(tree)).expect("write PSD");
    let back = PsdReader::from_bytes(&bytes).expect("read PSD back");

    let root = back.layers.root_layer_ids();
    assert_eq!(root.len(), 1, "group is the only root node");
    let group = back.layers.get(root[0]).expect("group present");
    assert_eq!(group.name, "Group A");
    let LayerContent::Group { ref children } = group.content else {
        panic!("expected a group, got {:?}", group.content);
    };
    assert_eq!(children.len(), 1);
    assert_eq!(back.layers.get(children[0]).expect("child").name, "Layer 1");
}

#[test]
fn rejects_oversized_canvas() {
    let tree = LayerTree::new(40_000, 8, 72.0, 72.0);
    assert!(matches!(
        PsdWriter::to_bytes(&document(tree)),
        Err(PsdError::DimensionsTooLarge { .. })
    ));
}
