// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Fixture generator for the malformed-input corpus in `tests/fixtures/`.
//!
//! Regenerate the corpus with:
//! `cargo test -p iris-aif --features gen-fixtures --test gen_fixtures`
//!
//! Each fixture is a deliberately malformed `.aif` container targeting one
//! `AifError` variant; `tests/malformed.rs` asserts the mapping. Fixtures are
//! checked in so normal test runs never write to the source tree (see the
//! `gen-fixtures` feature note in Cargo.toml).
#![cfg(feature = "gen-fixtures")]

use std::io::Cursor;

use iris_aif::{
    document::{AifArtboard, AifCanvas, AifDocument, CanvasMode},
    parts, AifWriter, WriteOptions,
};
use iris_pixel::{BitDepth, LayerTree};
use loki_opc::{Package, PartData, PartName};
use uuid::Uuid;

const LAYER_ID: &str = "00000000-0000-0000-0000-0000000000aa";

fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn write_fixture(name: &str, bytes: &[u8]) {
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).expect("create fixtures dir");
    std::fs::write(dir.join(name), bytes).expect("write fixture");
}

/// A valid empty pixel-mode document, serialised by the production writer.
fn base_package_bytes() -> Vec<u8> {
    let canvas = AifCanvas {
        mode: CanvasMode::Pixel,
        width_px: 256,
        height_px: 256,
        dpi_x: 72.0,
        dpi_y: 72.0,
        working_color_space: "linear-srgb".into(),
        bit_depth: BitDepth::F16,
    };
    let artboard = AifArtboard {
        id: Uuid::nil(),
        name: "Canvas".into(),
        x_px: 0,
        y_px: 0,
        width_px: 256,
        height_px: 256,
    };
    let doc = AifDocument {
        layers: LayerTree::new(256, 256, 72.0, 72.0),
        canvas,
        artboards: vec![artboard],
        format_version: (1, 0),
    };
    let opts = WriteOptions {
        doc_id: Uuid::nil(),
        created_at: "2024-01-01T00:00:00Z".into(),
        saved_at: "2024-01-01T00:00:00Z".into(),
        app_version: "0.1.0".into(),
    };
    let mut buf = Cursor::new(Vec::new());
    AifWriter::write(&doc, &mut buf, &opts).expect("write base doc");
    buf.into_inner()
}

/// Substitutable document.xml. Pass `""` for `canvas` to omit the element.
fn doc_xml(version: &str, canvas: &str, artboards: &str, layers: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<iris:Document xmlns:iris="https://appthere.dev/iris/2024"
    formatVersion="{version}" documentId="00000000-0000-0000-0000-000000000000"
    createdAt="2024-01-01T00:00:00Z" savedAt="2024-01-01T00:00:00Z" appVersion="0.1.0">
  {canvas}
  <iris:Artboards>{artboards}</iris:Artboards>
  <iris:LayerTree>{layers}</iris:LayerTree>
</iris:Document>"#
    )
    .into_bytes()
}

fn canvas_xml(width: &str, height: &str) -> String {
    format!(
        r#"<iris:Canvas mode="pixel" widthPx="{width}" heightPx="{height}"
       dpiX="72" dpiY="72" workingColorSpace="linear-srgb" bitDepth="f16" />"#
    )
}

fn artboard_xml(width: &str, height: &str) -> String {
    format!(
        r#"<iris:Artboard id="00000000-0000-0000-0000-000000000000" name="Canvas"
       xPx="0" yPx="0" widthPx="{width}" heightPx="{height}" />"#
    )
}

/// Replace a part inside the base package and return the repacked bytes.
fn with_part(base: &[u8], path: &str, bytes: Vec<u8>, media_type: &str) -> Vec<u8> {
    let mut pkg = Package::open(Cursor::new(base.to_vec())).expect("open base");
    let name = PartName::new(format!("/{path}")).expect("part name");
    pkg.set_part(name, PartData::new(bytes, media_type));
    let mut out = Cursor::new(Vec::new());
    pkg.write(&mut out, None).expect("repack");
    out.into_inner()
}

fn with_document_xml(base: &[u8], xml: Vec<u8>) -> Vec<u8> {
    with_part(base, parts::DOCUMENT_XML, xml, parts::CT_DOCUMENT)
}

/// meta.xml for a pixel layer with a substitutable blendMode and extra body.
fn layer_meta_xml(blend_mode: &str, body: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<iris:LayerMeta xmlns:iris="https://appthere.dev/iris/2024"
    id="{LAYER_ID}" type="pixel" name="layer" visible="true" locked="false"
    opacity="1" blendMode="{blend_mode}" clippingMask="false">
  <iris:PixelData tileSize="256" channelLayout="rgba" colorSpace="linear-srgb"
      bitDepth="f16" compression="zip" />
  {body}
</iris:LayerMeta>"#
    )
    .into_bytes()
}

/// A structurally valid EXR whose resolution is not the AIF tile size.
fn oversized_tile_exr() -> Vec<u8> {
    use exr::prelude::*;
    let layer = Layer::new(
        (512usize, 512usize),
        LayerAttributes::named(Text::from("rgba")),
        Encoding::FAST_LOSSLESS,
        SpecificChannels::rgba(|_: Vec2<usize>| {
            (f16::ONE, f16::ONE, f16::ONE, f16::ONE)
        }),
    );
    let mut buf = Cursor::new(Vec::new());
    Image::from_layer(layer)
        .write()
        .to_buffered(&mut buf)
        .expect("encode oversized exr");
    buf.into_inner()
}

fn single_layer_entry() -> String {
    format!(r#"<iris:Layer id="{LAYER_ID}" type="pixel" order="0" />"#)
}

#[test]
fn regenerate_fixture_corpus() {
    let base = base_package_bytes();
    let canvas = canvas_xml("256", "256");
    let artboard = artboard_xml("256", "256");

    // AifError::Opc — not a ZIP container at all.
    write_fixture("not_a_zip.aif", b"this is not a zip archive");

    // AifError::MissingRequiredPart — document.xml removed.
    let mut pkg = Package::open(Cursor::new(base.clone())).expect("open");
    pkg.remove_part(&PartName::new("/iris/document.xml").expect("name"));
    let mut out = Cursor::new(Vec::new());
    pkg.write(&mut out, None).expect("repack");
    write_fixture("missing_document_xml.aif", &out.into_inner());

    // AifError::UnsupportedMajorVersion — formatVersion 99.0.
    let xml = doc_xml("99.0", &canvas, &artboard, "");
    write_fixture("unsupported_major_version.aif", &with_document_xml(&base, xml));

    // AifError::XmlParse — truncated, unparseable document.xml.
    write_fixture(
        "malformed_document_xml.aif",
        &with_document_xml(&base, b"<iris:Document".to_vec()),
    );

    // AifError::MissingAttribute — no <iris:Canvas> element at all.
    let xml = doc_xml("1.0", "", &artboard, "");
    write_fixture("missing_canvas.aif", &with_document_xml(&base, xml));

    // AifError::XmlParse — canvas width above MAX_CANVAS_DIMENSION.
    let xml = doc_xml("1.0", &canvas_xml("4000000", "256"), &artboard, "");
    write_fixture("oversized_canvas.aif", &with_document_xml(&base, xml));

    // AifError::InvalidPixelModeArtboard — pixel mode with zero artboards.
    let xml = doc_xml("1.0", &canvas, "", "");
    write_fixture("pixel_mode_no_artboard.aif", &with_document_xml(&base, xml));

    // AifError::DuplicateLayerId — the same layer UUID listed twice.
    let dup = format!("{}{}", single_layer_entry(), single_layer_entry());
    let xml = doc_xml("1.0", &canvas, &artboard, &dup);
    write_fixture("duplicate_layer_id.aif", &with_document_xml(&base, xml));

    // AifError::MissingLayerMeta — layer entry with no meta.xml part.
    let xml = doc_xml("1.0", &canvas, &artboard, &single_layer_entry());
    write_fixture("missing_layer_meta.aif", &with_document_xml(&base, xml));

    // AifError::UnknownBlendMode — meta.xml with an unrecognised blend mode.
    let layer_id: Uuid = LAYER_ID.parse().expect("uuid");
    let xml = doc_xml("1.0", &canvas, &artboard, &single_layer_entry());
    let with_layer = with_document_xml(&base, xml);
    let meta = layer_meta_xml("frobnicate", "");
    let meta_path = parts::layer_meta_xml(&layer_id);
    write_fixture(
        "unknown_blend_mode.aif",
        &with_part(&with_layer, &meta_path, meta, "application/xml"),
    );

    // AifError::XmlParse — CropBounds far above MAX_CANVAS_DIMENSION would
    // otherwise drive the reader's tile loop for ~2^44 iterations.
    let crop = r#"<iris:CropBounds xPx="0" yPx="0" widthPx="4294967295" heightPx="4294967295" />"#;
    let meta = layer_meta_xml("normal", crop);
    write_fixture(
        "oversized_crop_bounds.aif",
        &with_part(&with_layer, &meta_path, meta, "application/xml"),
    );

    // AifError::TileReadError — tile part is not an EXR file.
    let meta = layer_meta_xml("normal", "");
    let with_meta = with_part(&with_layer, &meta_path, meta, "application/xml");
    let tile_path = parts::layer_tile_exr(&layer_id, 0, 0);
    write_fixture(
        "corrupt_tile.aif",
        &with_part(&with_meta, &tile_path, b"garbage, not an EXR".to_vec(), parts::CT_EXR),
    );

    // AifError::TileReadError — valid EXR with a non-tile resolution (512×512);
    // regression test for the unchecked pixel-position write in tile.rs.
    write_fixture(
        "oversized_tile.aif",
        &with_part(&with_meta, &tile_path, oversized_tile_exr(), parts::CT_EXR),
    );
}
