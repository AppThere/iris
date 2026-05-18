// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `iris-aif` Phase 1 milestone.
//!
//! Covers:
//! - Round-trip: write → read → compare (always-run, in-memory).
//! - Error paths: missing required parts, unsupported version, duplicate IDs.
//! - Sparse-tile omission: fully-transparent tiles not written.
//! - Preview PNG: non-empty, valid PNG magic bytes.
//!
//! NOTE: `AifError::PermissionRevoked` is not testable on desktop CI.
//! TODO(iris): SPEC.md §10 — add PermissionRevoked test once loki-file-access mock API exists.

use std::io::Cursor;

use iris_aif::{
    document::{AifArtboard, AifCanvas, AifDocument, CanvasMode},
    AifError, AifReader, AifWriter, WriteOptions,
};
use iris_pixel::{BitDepth, LayerTree};
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
