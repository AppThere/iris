// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Cross-crate integration smoke test: iris-ops + iris-pixel + iris-aif.
//!
//! Verifies the full write → read round-trip for a two-layer pixel document
//! and exercises the undo stack with a TileOp.

use std::io::{Seek, SeekFrom};

use iris_aif::{
    document::{AifArtboard, AifCanvas, AifDocument, CanvasMode},
    parts, AifReader, AifWriter, WriteOptions,
};
use iris_ops::{Op, TileOp, TileSnapshot, UndoStack};
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerTree,
    PixelLayer, TileCache, TileCoord, TileData, TILE_SIZE, LINEAR_SRGB,
};
use loki_opc::PartName;
use uuid::Uuid;

// f16 1.0 in little-endian.
const F16_ONE_LE: [u8; 2] = [0x00, 0x3C];

fn opaque_white_tile() -> TileData {
    let n = TILE_SIZE as usize;
    let pixel: [u8; 8] = [
        F16_ONE_LE[0], F16_ONE_LE[1], // R = 1.0
        F16_ONE_LE[0], F16_ONE_LE[1], // G = 1.0
        F16_ONE_LE[0], F16_ONE_LE[1], // B = 1.0
        F16_ONE_LE[0], F16_ONE_LE[1], // A = 1.0 (opaque)
    ];
    let mut bytes = vec![0u8; n * n * 8];
    for chunk in bytes.chunks_exact_mut(8) {
        chunk.copy_from_slice(&pixel);
    }
    TileData::from_vec(bytes)
}

fn new_pixel_layer(id: Uuid) -> Layer {
    Layer {
        id,
        name: "layer".into(),
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
            tiles: TileCache::new(16),
        }),
    }
}

#[test]
fn integration_smoke() {
    // 1. Create a LayerTree (canvas 800×600, 96 dpi).
    let mut layers = LayerTree::new(800, 600, 96.0, 96.0);

    // 2. Add two pixel layers (RGBA, f16, linear-sRGB, ZIP compression).
    let layer1_id = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);
    let layer2_id = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0002);
    layers.add_layer(None, 0, new_pixel_layer(layer1_id)).expect("add layer1");
    layers.add_layer(None, 1, new_pixel_layer(layer2_id)).expect("add layer2");

    // 3. Write non-transparent pixel data to tile (0,0) of layer 1.
    let coord = TileCoord { tx: 0, ty: 0 };
    let tile_data = opaque_white_tile();
    {
        let layer1 = layers.get_mut(layer1_id).expect("layer1 must exist");
        let LayerContent::Pixel(ref mut px) = layer1.content else {
            panic!("layer1 must be a pixel layer");
        };
        px.tiles.insert(coord, tile_data.clone());
    }

    // 4. Push a TileOp onto an UndoStack.
    let mut undo_stack = UndoStack::new(200);
    let blank = vec![0u8; TILE_SIZE as usize * TILE_SIZE as usize * 8];
    let op = Op::Tile(TileOp::Paint {
        layer_id: layer1_id,
        tile: coord,
        before: TileSnapshot::compress(&blank),
        after: TileSnapshot::compress(tile_data.bytes()),
    });
    undo_stack.push(op);

    // 5. Write the document to a tempfile as .aif.
    let canvas = AifCanvas {
        mode: CanvasMode::Pixel,
        width_px: 800,
        height_px: 600,
        dpi_x: 96.0,
        dpi_y: 96.0,
        working_color_space: "linear-srgb".into(),
        bit_depth: BitDepth::F16,
    };
    let artboard = AifArtboard {
        id: Uuid::from_u128(0xFF),
        name: "Canvas".into(),
        x_px: 0,
        y_px: 0,
        width_px: 800,
        height_px: 600,
    };
    let doc = AifDocument { canvas, artboards: vec![artboard], layers, format_version: (1, 0) };
    let opts = WriteOptions {
        doc_id: Uuid::from_u128(0xDEAD),
        created_at: "2024-01-01T00:00:00Z".into(),
        saved_at: "2024-01-01T00:00:00Z".into(),
        app_version: "0.1.0".into(),
    };

    let mut tmp = tempfile::tempfile().expect("create tempfile");
    AifWriter::write(&doc, &mut tmp, &opts).expect("write failed");
    tmp.seek(SeekFrom::Start(0)).expect("seek to start");

    // 6. Read it back with AifReader::open.
    let loaded = AifReader::open(&mut tmp).expect("read failed");

    // 7a. Assert canvas dimensions match (800×600).
    assert_eq!(loaded.canvas.width_px, 800, "canvas width must be 800");
    assert_eq!(loaded.canvas.height_px, 600, "canvas height must be 600");

    // 7b. Assert layer count matches (2).
    assert_eq!(loaded.layers.root_layer_ids().len(), 2, "must have 2 layers");

    // 7c. Assert tile (0,0) of layer 1 bytes match the written data.
    let loaded_layer1 = loaded.layers.get(layer1_id).expect("layer1 must be in loaded doc");
    let LayerContent::Pixel(ref px1) = loaded_layer1.content else {
        panic!("loaded layer1 must be a pixel layer");
    };
    let loaded_tile = px1.tiles.get(coord).expect("tile (0,0) of layer1 must be present");
    assert_eq!(
        loaded_tile.bytes(),
        tile_data.bytes(),
        "tile bytes must round-trip exactly"
    );

    // 7d. Assert tile (0,0) of layer 2 is absent from the OPC package (sparse).
    tmp.seek(SeekFrom::Start(0)).expect("seek to start for OPC check");
    let pkg = loki_opc::Package::open(&mut tmp).expect("open OPC package");
    let tile2_path = format!("/{}", parts::layer_tile_exr(&layer2_id, 0, 0));
    let tile2_name = PartName::new(tile2_path).expect("valid part name");
    assert!(
        pkg.part(&tile2_name).is_none(),
        "layer2 tile (0,0) must be absent from OPC package (sparse tile omission)"
    );

    // 8. Undo the TileOp; assert UndoStack::can_redo() is true.
    undo_stack.undo();
    assert!(undo_stack.can_redo(), "after undo, can_redo must be true");
}
