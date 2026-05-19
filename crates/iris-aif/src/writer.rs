// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! `AifWriter` — serialises an [`AifDocument`] to an `.aif` OPC package.
//!
//! Entry points:
//! - [`AifWriter::write`] — to any `Write + Seek` sink.
//! - [`AifWriter::write_token`] — sandboxed platforms via [`FileAccessToken`].
//!
//! Compression strategy (§4.3): `.exr` parts use `Stored` (EXR carries its
//! own codec); all other parts use `Deflated`.

use std::io::{Seek, Write};

use loki_opc::{Package, PartData, PartName};
use uuid::Uuid;

use crate::{
    document::AifDocument,
    error::AifError,
    meta::serialise_layer_meta,
    parts,
    preview::write_preview_png,
    tile::write_tile_exr,
    xml::{write_document_xml, write_metadata_xml, DocumentMetadata, SessionInfo},
    FileAccessToken,
};
use iris_pixel::{LayerContent, LayerTree};

// ── Public options / API ──────────────────────────────────────────────────────

/// Controls serialisation behaviour.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Document UUID stamped into `document.xml`.
    pub doc_id: Uuid,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 save timestamp.
    pub saved_at: String,
    /// Application version string (e.g. `"0.1.0"`).
    pub app_version: String,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            doc_id: Uuid::new_v4(),
            created_at: "1970-01-01T00:00:00Z".into(),
            saved_at: "1970-01-01T00:00:00Z".into(),
            app_version: "0.1.0".into(),
        }
    }
}

/// Serialises AIF documents to files or byte streams.
pub struct AifWriter;

impl AifWriter {
    /// Write `doc` to any `Write + Seek` sink using the given options.
    pub fn write<W: Write + Seek>(
        doc: &AifDocument,
        sink: W,
        opts: &WriteOptions,
    ) -> Result<(), AifError> {
        let pkg = build_package(doc, opts)?;
        let compression_cb = |name: &PartName| {
            if name.as_str().ends_with(".exr") {
                loki_opc::CompressionMethod::Stored
            } else {
                loki_opc::CompressionMethod::Deflated
            }
        };
        pkg.write(sink, Some(&compression_cb))?;
        Ok(())
    }

    /// Write `doc` to a [`FileAccessToken`] destination (sandboxed platforms).
    pub fn write_token(
        doc: &AifDocument,
        token: FileAccessToken,
        opts: &WriteOptions,
    ) -> Result<(), AifError> {
        let writer = token
            .open_write_truncate()
            .map_err(|e| AifError::FileAccess { source: e })?;
        Self::write(doc, writer, opts)
    }
}

// ── Package construction ──────────────────────────────────────────────────────

fn build_package(doc: &AifDocument, opts: &WriteOptions) -> Result<Package, AifError> {
    let mut pkg = Package::new();

    // Register content-type defaults for all non-XML extensions used in AIF
    // so that loki-opc's strict reader can resolve them from [Content_Types].xml.
    let ctm = pkg.content_type_map_mut();
    ctm.add_default("exr", parts::CT_EXR);
    ctm.add_default("png", parts::CT_PNG);
    ctm.add_default("bin", parts::CT_OPLOG);

    add_document_xml(&mut pkg, doc, opts)?;
    add_metadata_xml(&mut pkg)?;
    add_layer_parts(&mut pkg, &doc.layers)?;
    add_preview_png(&mut pkg)?;
    add_ops_stub(&mut pkg)?;

    Ok(pkg)
}

fn add_document_xml(
    pkg: &mut Package,
    doc: &AifDocument,
    opts: &WriteOptions,
) -> Result<(), AifError> {
    let bytes = write_document_xml(
        &doc.canvas,
        &doc.artboards,
        &doc.layers,
        opts.doc_id,
        &opts.app_version,
        &opts.created_at,
        &opts.saved_at,
    )?;
    set_part(pkg, parts::DOCUMENT_XML, bytes, parts::CT_DOCUMENT)
}

fn add_metadata_xml(pkg: &mut Package) -> Result<(), AifError> {
    let meta = DocumentMetadata {
        title: None,
        author: None,
        description: None,
        keywords: None,
    };
    let session = SessionInfo {
        started_at: "1970-01-01T00:00:00Z".into(),
        ended_at: "1970-01-01T00:00:00Z".into(),
        app_version: "0.1.0".into(),
        platform: "unknown".into(),
    };
    let bytes = write_metadata_xml(&meta, &session)?;
    set_part(pkg, parts::METADATA_XML, bytes, parts::CT_METADATA)
}

fn add_layer_parts(pkg: &mut Package, tree: &LayerTree) -> Result<(), AifError> {
    for layer in tree.iter_depth_first() {
        let meta_bytes = serialise_layer_meta(layer)?;
        let meta_path = parts::layer_meta_xml(&layer.id);
        set_part(pkg, &meta_path, meta_bytes, "application/xml")?;

        if let LayerContent::Pixel(ref px) = layer.content {
            for (coord, tile_data) in px.tiles.dirty_coords().collect::<Vec<_>>().into_iter()
                .filter_map(|c| px.tiles.get(c).map(|t| (c, t)))
            {
                if tile_data.is_fully_transparent() {
                    continue; // sparse tile — omit per §4.3
                }
                let exr_bytes = write_tile_exr(tile_data, layer.id, coord.tx, coord.ty)?;
                let tile_path = parts::layer_tile_exr(&layer.id, coord.tx, coord.ty);
                set_part(pkg, &tile_path, exr_bytes, parts::CT_EXR)?;
            }
        }
    }
    Ok(())
}

fn add_preview_png(pkg: &mut Package) -> Result<(), AifError> {
    let bytes = write_preview_png()?;
    set_part(pkg, parts::PREVIEW_PNG, bytes, parts::CT_PNG)
}

fn add_ops_stub(pkg: &mut Package) -> Result<(), AifError> {
    // 12-byte stub: magic "AIROPLOG" + little-endian u32 version=1.
    let mut stub = b"AIROPLOG".to_vec();
    stub.extend_from_slice(&1u32.to_le_bytes());
    set_part(pkg, parts::HISTORY_OPS_BIN, stub, parts::CT_OPLOG)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn set_part(
    pkg: &mut Package,
    path: &str,
    bytes: Vec<u8>,
    media_type: &str,
) -> Result<(), AifError> {
    let name = PartName::new(format!("/{path}")).map_err(AifError::Opc)?;
    pkg.set_part(name, PartData::new(bytes, media_type));
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{AifArtboard, AifCanvas, CanvasMode};
    use iris_pixel::{BitDepth, LayerTree};

    fn minimal_doc() -> AifDocument {
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
        AifDocument {
            layers: LayerTree::new(256, 256, 72.0, 72.0),
            canvas,
            artboards: vec![artboard],
            format_version: (1, 0),
        }
    }

    #[test]
    fn write_to_vec_does_not_error() {
        use std::io::Cursor;
        let doc = minimal_doc();
        let opts = WriteOptions::default();
        let mut buf = Cursor::new(Vec::new());
        AifWriter::write(&doc, &mut buf, &opts).expect("write should succeed");
        assert!(!buf.into_inner().is_empty());
    }
}
