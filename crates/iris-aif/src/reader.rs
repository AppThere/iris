// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! `AifReader` — loads an `.aif` package into an [`AifDocument`].
//!
//! Entry points:
//! - [`AifReader::open`] — from any `Read + Seek` source (desktop / tests).
//! - [`AifReader::open_token`] — sandboxed platforms via [`FileAccessToken`].
//!
//! Orchestration order per SPEC.md §4.2:
//! 1. Parse `iris/document.xml` → canvas, artboards, layer-tree entries.
//! 2. For each entry: parse `iris/layers/{id}/meta.xml` → `LayerMetaSpec`.
//! 3. Load EXR tiles into each `PixelLayer`'s `TileCache`.
//! 4. Insert layers into `LayerTree` in document order.

use std::io::{Read, Seek};

use loki_opc::{Package, PartName};
use uuid::Uuid;

use crate::{
    document::{AifCanvas, AifDocument},
    error::AifError,
    meta::{parse_layer_meta, spec_to_layer},
    parts,
    paths::read_path_store,
    tile::read_tile_exr,
    xml::{read_document_xml, read_metadata_xml, LayerTreeEntry},
    FileAccessToken,
};
use iris_pixel::{LayerContent, LayerTree, PixelLayer, TileCoord, VectorLayer, TILE_SIZE};

// ── Public API ────────────────────────────────────────────────────────────────

/// Reads AIF packages from files, byte streams, or sandboxed file tokens.
pub struct AifReader;

impl AifReader {
    /// Open an AIF package from any `Read + Seek` source.
    ///
    /// Suitable for desktop use and unit tests.  On sandboxed platforms
    /// (iOS, Android) use [`open_token`][Self::open_token] instead.
    pub fn open(source: impl Read + Seek) -> Result<AifDocument, AifError> {
        let pkg = Package::open(source)?;
        read_package(pkg)
    }

    /// Open an AIF package via a [`FileAccessToken`] (sandboxed platforms).
    pub fn open_token(token: FileAccessToken) -> Result<AifDocument, AifError> {
        let reader = token
            .open_read()
            .map_err(|e| AifError::FileAccess { source: e })?;
        let pkg = Package::open(reader)?;
        read_package(pkg)
    }
}

// ── Package orchestration ─────────────────────────────────────────────────────

fn read_package(pkg: Package) -> Result<AifDocument, AifError> {
    // 1 — document.xml
    let doc_bytes = required_part(&pkg, parts::DOCUMENT_XML)?;
    let (canvas, artboards, entries) = read_document_xml(doc_bytes)?;

    // Optional: validate metadata.xml if present (errors are non-fatal per §4.16 rule 4).
    if let Ok(meta_bytes) = required_part(&pkg, parts::METADATA_XML) {
        let _ = read_metadata_xml(meta_bytes);
    }

    // 2 — build LayerTree
    let mut tree = LayerTree::new(
        canvas.width_px,
        canvas.height_px,
        canvas.dpi_x,
        canvas.dpi_y,
    );

    let format_version = parse_format_version_from_pkg(&pkg);

    // 3 — load each layer from meta.xml + tiles
    load_layers(&pkg, &mut tree, &entries, &canvas)?;

    Ok(AifDocument {
        canvas,
        artboards,
        layers: tree,
        format_version,
    })
}

fn load_layers(
    pkg: &Package,
    tree: &mut LayerTree,
    entries: &[LayerTreeEntry],
    _canvas: &AifCanvas,
) -> Result<(), AifError> {
    for entry in entries {
        let meta_path = parts::layer_meta_xml(&entry.id);
        let meta_bytes = required_part_by_id(pkg, &meta_path, entry.id)?;
        let spec = parse_layer_meta(meta_bytes, entry.id)?;
        let mut layer = spec_to_layer(spec);

        match layer.content {
            // Load EXR tiles for pixel layers.
            LayerContent::Pixel(ref mut px) => load_pixel_tiles(pkg, &entry.id, px)?,
            // Load FlatBuffers geometry for vector layers (§4.10).
            LayerContent::Vector(ref mut vl) => *vl = load_path_store(pkg, entry.id)?,
            _ => {}
        }

        tree.add_layer(None, entry.order as usize, layer)
            .map_err(|_| AifError::MissingLayerMeta { layer_id: entry.id })?;
    }
    Ok(())
}

/// Load a vector layer's `paths.bin` (§4.10).
///
/// A vector layer's entire content lives in this part, so its absence is data
/// loss rather than an empty layer — §4.1 rule 2 requires failing loudly rather
/// than silently producing a layer with no geometry.
fn load_path_store(pkg: &Package, layer_id: Uuid) -> Result<VectorLayer, AifError> {
    let path = parts::layer_paths_bin(&layer_id);
    let part_name = make_part_name(&path)?;
    let part = pkg
        .part(&part_name)
        .ok_or_else(|| AifError::MissingRequiredPart(path.clone()))?;
    // AUDIT: SPEC.md §4.6 defines no colorSpace attribute for vector layers, so
    // there is nothing here to cross-check against PathStore.colorSpace. See the
    // note on `read_path_store`.
    read_path_store(&part.bytes, layer_id, None)
}

fn load_pixel_tiles(
    pkg: &Package,
    layer_id: &Uuid,
    px: &mut PixelLayer,
) -> Result<(), AifError> {
    let (tx_count, ty_count) = pixel_tile_dims(px);

    for ty in 0..ty_count {
        for tx in 0..tx_count {
            let tile_path = parts::layer_tile_exr(layer_id, tx, ty);
            let part_name = make_part_name(&tile_path)?;
            if let Some(part) = pkg.part(&part_name) {
                let tile_data = read_tile_exr(&part.bytes, *layer_id, tx, ty)?;
                px.tiles.insert(TileCoord { tx, ty }, tile_data);
            }
            // Missing tile → transparent; TileCache returns None (treated as zeros).
        }
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Number of tiles in each dimension for a pixel layer.
fn pixel_tile_dims(px: &PixelLayer) -> (u32, u32) {
    // Use crop_bounds if available, otherwise assume a single tile.
    match &px.crop_bounds {
        Some(b) => (
            PixelLayer::tiles_needed(b.width),
            PixelLayer::tiles_needed(b.height),
        ),
        None => (
            PixelLayer::tiles_needed(TILE_SIZE),
            PixelLayer::tiles_needed(TILE_SIZE),
        ),
    }
}

fn required_part<'p>(pkg: &'p Package, path: &str) -> Result<&'p [u8], AifError> {
    let name = make_part_name(path)?;
    pkg.part(&name)
        .map(|p| p.bytes.as_slice())
        .ok_or_else(|| AifError::MissingRequiredPart(path.into()))
}

fn required_part_by_id<'p>(
    pkg: &'p Package,
    path: &str,
    layer_id: Uuid,
) -> Result<&'p [u8], AifError> {
    let name = make_part_name(path)?;
    pkg.part(&name)
        .map(|p| p.bytes.as_slice())
        .ok_or(AifError::MissingLayerMeta { layer_id })
}

fn make_part_name(path: &str) -> Result<PartName, AifError> {
    PartName::new(format!("/{path}")).map_err(AifError::Opc)
}

fn parse_format_version_from_pkg(_pkg: &Package) -> (u32, u32) {
    // formatVersion is already validated in read_document_xml; we return
    // the library's own version here as the "as-written" version.
    (crate::parts::AIF_MAJOR, crate::parts::AIF_MINOR)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_part_name_prepends_slash() {
        let name = make_part_name(parts::DOCUMENT_XML).expect("ok");
        assert!(name.as_str().starts_with('/'));
    }
}
