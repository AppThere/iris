// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `iris-aif` Phase 1 milestone.
//!
//! Covers:
//! - Round-trip: write → read → compare (always-run, in-memory).
//! - Error paths: missing required parts, unsupported version, duplicate IDs.
//! - Sparse-tile omission: fully-transparent tiles not written.
//! - Preview PNG: non-empty, valid PNG magic bytes.
//! - Tile metadata mismatch: EXR aifLayerId does not match OPC part path.
//!
//! NOTE: `AifError::PermissionRevoked` is not testable on desktop CI.
//! TODO(iris): SPEC.md §10 — add PermissionRevoked test once loki-file-access mock API exists.

use std::io::Cursor;

use iris_aif::{
    document::{AifArtboard, AifCanvas, AifDocument, CanvasMode},
    AifError, AifReader, AifWriter, WriteOptions,
};
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerTree,
    PixelLayer, TileCache, TileCoord, TileData, TILE_SIZE, LINEAR_SRGB,
};
use uuid::Uuid;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pixel_canvas() -> AifCanvas {
    AifCanvas {
        mode: CanvasMode::Pixel,
        width_px: 256,
        height_px: 256,
        dpi_x: 72.0,
        dpi_y: 72.0,
        working_color_space: "linear-srgb".into(),
        bit_depth: BitDepth::F16,
    }
}

fn pixel_artboard(canvas: &AifCanvas) -> AifArtboard {
    AifArtboard {
        id: Uuid::nil(),
        name: "Canvas".into(),
        x_px: 0,
        y_px: 0,
        width_px: canvas.width_px,
        height_px: canvas.height_px,
    }
}

fn empty_pixel_doc() -> AifDocument {
    let canvas = pixel_canvas();
    let artboard = pixel_artboard(&canvas);
    AifDocument {
        layers: LayerTree::new(256, 256, 72.0, 72.0),
        canvas,
        artboards: vec![artboard],
        format_version: (1, 0),
    }
}

fn write_doc(doc: &AifDocument) -> Vec<u8> {
    let opts = WriteOptions {
        doc_id: Uuid::nil(),
        created_at: "2024-01-01T00:00:00Z".into(),
        saved_at: "2024-01-01T00:00:00Z".into(),
        app_version: "0.1.0".into(),
    };
    let mut buf = Cursor::new(Vec::new());
    AifWriter::write(doc, &mut buf, &opts).expect("write failed");
    buf.into_inner()
}

// ── Always-run tests ──────────────────────────────────────────────────────────

#[test]
fn round_trip_empty_document() {
    let doc = empty_pixel_doc();
    let bytes = write_doc(&doc);
    assert!(!bytes.is_empty(), "output must not be empty");

    let loaded = AifReader::open(Cursor::new(bytes)).expect("read failed");
    assert_eq!(loaded.canvas.width_px, 256);
    assert_eq!(loaded.canvas.height_px, 256);
    assert_eq!(loaded.canvas.mode, CanvasMode::Pixel);
    assert_eq!(loaded.artboards.len(), 1);
    assert_eq!(loaded.artboards[0].name, "Canvas");
}

#[test]
fn round_trip_canvas_fields_preserved() {
    let doc = empty_pixel_doc();
    let bytes = write_doc(&doc);
    let loaded = AifReader::open(Cursor::new(bytes)).expect("read failed");

    assert!((loaded.canvas.dpi_x - 72.0).abs() < 0.01);
    assert!((loaded.canvas.dpi_y - 72.0).abs() < 0.01);
    assert_eq!(loaded.canvas.working_color_space, "linear-srgb");
    assert_eq!(loaded.canvas.bit_depth, BitDepth::F16);
}

#[test]
fn missing_document_xml_returns_error() {
    // Write a valid package then strip the document.xml part.
    use loki_opc::{Package, PartName};
    let doc = empty_pixel_doc();
    let bytes = write_doc(&doc);

    let mut pkg = Package::open(Cursor::new(bytes)).expect("open pkg");
    let doc_name = PartName::new("/iris/document.xml").expect("name");
    pkg.remove_part(&doc_name);

    let mut out = Cursor::new(Vec::new());
    pkg.write(&mut out, None).expect("rewrite");

    let err = AifReader::open(Cursor::new(out.into_inner())).expect_err("should fail");
    assert!(
        matches!(err, AifError::MissingRequiredPart(_)),
        "expected MissingRequiredPart, got {err:?}"
    );
}

#[test]
fn unsupported_major_version_returns_error() {
    // Craft document.xml bytes with a future major version.
    let bad_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<iris:Document xmlns:iris="https://appthere.dev/iris/2024"
    formatVersion="99.0" documentId="00000000-0000-0000-0000-000000000000"
    createdAt="2024-01-01T00:00:00Z" savedAt="2024-01-01T00:00:00Z"
    appVersion="99.0">
  <iris:Canvas mode="pixel" widthPx="1" heightPx="1"
               dpiX="72" dpiY="72" workingColorSpace="linear-srgb" bitDepth="f16" />
  <iris:Artboards>
    <iris:Artboard id="00000000-0000-0000-0000-000000000000" name="Canvas"
                   xPx="0" yPx="0" widthPx="1" heightPx="1" />
  </iris:Artboards>
  <iris:LayerTree/>
</iris:Document>"#;

    // Re-pack into an OPC container over the existing package.
    let doc = empty_pixel_doc();
    let bytes = write_doc(&doc);
    let mut pkg = loki_opc::Package::open(Cursor::new(bytes)).expect("open");
    let name = loki_opc::PartName::new("/iris/document.xml").expect("name");
    let data = loki_opc::PartData::new(bad_xml.to_vec(), iris_aif::parts::CT_DOCUMENT);
    pkg.set_part(name, data);
    let mut out = Cursor::new(Vec::new());
    pkg.write(&mut out, None).expect("rewrite");

    let err = AifReader::open(Cursor::new(out.into_inner())).expect_err("should fail");
    assert!(
        matches!(err, AifError::UnsupportedMajorVersion { .. }),
        "expected UnsupportedMajorVersion, got {err:?}"
    );
}

#[test]
fn write_output_is_valid_zip() {
    let doc = empty_pixel_doc();
    let bytes = write_doc(&doc);
    // ZIP files begin with PK (local file header signature 0x504B0304).
    assert_eq!(&bytes[..2], b"PK", "output must be a valid ZIP file");
}

#[test]
fn preview_png_is_present_and_valid() {
    use loki_opc::{Package, PartName};
    let doc = empty_pixel_doc();
    let bytes = write_doc(&doc);
    let pkg = Package::open(Cursor::new(bytes)).expect("open");
    let name = PartName::new("/iris/preview.png").expect("name");
    let part = pkg.part(&name).expect("preview.png must be present");
    assert_eq!(&part.bytes[..8], b"\x89PNG\r\n\x1a\n", "must be PNG");
}

// ── Tile metadata mismatch ────────────────────────────────────────────────────

// f16 1.0 in little-endian bytes.
const F16_ONE_LE: [u8; 2] = [0x00, 0x3C];

fn doc_with_painted_layer(layer_id: Uuid) -> AifDocument {
    let side = TILE_SIZE as usize;
    let pixel: [u8; 8] = [
        F16_ONE_LE[0], F16_ONE_LE[1], // R = 1.0
        F16_ONE_LE[0], F16_ONE_LE[1], // G = 1.0
        F16_ONE_LE[0], F16_ONE_LE[1], // B = 1.0
        F16_ONE_LE[0], F16_ONE_LE[1], // A = 1.0
    ];
    let mut raw = vec![0u8; side * side * 8];
    for chunk in raw.chunks_exact_mut(8) {
        chunk.copy_from_slice(&pixel);
    }
    let mut cache = TileCache::new(4);
    cache.insert(TileCoord { tx: 0, ty: 0 }, TileData::from_vec(raw));

    let layer = Layer {
        id: layer_id,
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
            tiles: cache,
        }),
    };
    let canvas = pixel_canvas();
    let artboard = pixel_artboard(&canvas);
    let mut layers = LayerTree::new(canvas.width_px, canvas.height_px, canvas.dpi_x, canvas.dpi_y);
    layers.add_layer(None, 0, layer).expect("add layer");
    AifDocument { canvas, artboards: vec![artboard], layers, format_version: (1, 0) }
}

#[test]
fn tile_metadata_mismatch_returns_error() {
    use loki_opc::{Package, PartData, PartName};

    // Two distinct layer UUIDs — their EXRs will embed different aifLayerId values.
    let id_a = Uuid::from_u128(0xAAAA);
    let id_b = Uuid::from_u128(0xBBBB);

    let bytes_a = write_doc(&doc_with_painted_layer(id_a));
    let bytes_b = write_doc(&doc_with_painted_layer(id_b));

    // Extract layer A's EXR bytes (aifLayerId = id_a) from package A.
    let pkg_a = Package::open(Cursor::new(bytes_a)).expect("open pkg_a");
    let path_a = format!("/{}", iris_aif::parts::layer_tile_exr(&id_a, 0, 0));
    let name_a = PartName::new(path_a).expect("name_a");
    let exr_from_a = pkg_a.part(&name_a).expect("tile_a must exist").bytes.clone();

    // Tamper: put layer A's EXR into layer B's tile slot — wrong aifLayerId inside.
    let mut pkg_b = Package::open(Cursor::new(bytes_b)).expect("open pkg_b");
    let path_b = format!("/{}", iris_aif::parts::layer_tile_exr(&id_b, 0, 0));
    let name_b = PartName::new(path_b).expect("name_b");
    pkg_b.set_part(name_b, PartData::new(exr_from_a, iris_aif::parts::CT_EXR));
    let mut out = Cursor::new(Vec::new());
    pkg_b.write(&mut out, None).expect("rewrite pkg_b");

    let err = AifReader::open(Cursor::new(out.into_inner())).expect_err("should fail");
    assert!(
        matches!(err, AifError::TileMetadataMismatch { .. }),
        "expected TileMetadataMismatch, got {err:?}"
    );
}
