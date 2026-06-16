// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Round-trip and compatibility tests for `iris-ora`.
//!
//! Documents are built with iris-aif/iris-pixel, written with [`OraWriter`],
//! and read back with [`OraReader`].

use std::io::{Cursor, Write};

use iris_aif::{layer_from_rgba8, layer_to_rgba8, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BitDepth, BlendMode, Layer, LayerContent, LayerTree};
use iris_ora::{OraError, OraReader, OraWriter};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn swatch() -> Vec<u8> {
    vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255]
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
fn round_trips_group_and_pixel_layer() {
    let mut tree = LayerTree::new(4, 4, 72.0, 72.0);
    let gid = tree.add_layer(None, 0, group_node("Group A")).expect("add group");
    let mut child = layer_from_rgba8(2, 2, &swatch(), "Layer 1");
    if let LayerContent::Pixel(ref mut px) = child.content {
        px.canvas_offset_x = 1;
        px.canvas_offset_y = 1;
    }
    child.blend_mode = BlendMode::Multiply;
    child.opacity = 0.5;
    tree.add_layer(Some(gid), 0, child).expect("add child");

    let bytes = OraWriter::to_bytes(&document(tree)).expect("write ORA");
    let back = OraReader::from_bytes(&bytes).expect("read ORA back");

    assert_eq!(back.canvas.width_px, 4);
    let root = back.layers.root_layer_ids();
    assert_eq!(root.len(), 1, "the group is the only root node");

    let group = back.layers.get(root[0]).expect("group present");
    assert_eq!(group.name, "Group A");
    let LayerContent::Group { ref children } = group.content else {
        panic!("expected group, got {:?}", group.content);
    };
    assert_eq!(children.len(), 1);

    let child = back.layers.get(children[0]).expect("child present");
    assert_eq!(child.name, "Layer 1");
    assert_eq!(child.blend_mode, BlendMode::Multiply);
    assert!((child.opacity - 0.5).abs() < 0.01);

    let px = layer_to_rgba8(child).expect("pixels");
    assert_eq!((px.offset_x, px.offset_y), (1, 1), "layer position preserved");
    assert!(px.rgba[0] > 200 && px.rgba[1] < 50, "top-left pixel red");
}

#[test]
fn round_trips_krita_composite_op() {
    // COMPAT(krita): modes outside the SVG core set use Krita's vendor prefix.
    let mut tree = LayerTree::new(2, 2, 72.0, 72.0);
    let mut layer = layer_from_rgba8(2, 2, &swatch(), "Sub");
    layer.blend_mode = BlendMode::Subtract;
    tree.add_layer(None, 0, layer).expect("add");

    let bytes = OraWriter::to_bytes(&document(tree)).expect("write");
    let back = OraReader::from_bytes(&bytes).expect("read");
    let root = back.layers.root_layer_ids();
    assert_eq!(back.layers.get(root[0]).unwrap().blend_mode, BlendMode::Subtract);
}

#[test]
fn rejects_missing_mimetype() {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    zw.start_file("stack.xml", SimpleFileOptions::default()).expect("start");
    zw.write_all(b"<image w=\"1\" h=\"1\"><stack/></image>").expect("write");
    let bytes = zw.finish().expect("finish").into_inner();

    assert!(matches!(OraReader::from_bytes(&bytes), Err(OraError::BadMimetype)));
}
