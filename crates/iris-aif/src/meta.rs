// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Per-layer `meta.xml` I/O helpers used by `reader.rs` and `writer.rs`.
//!
//! Thin wrappers over [`crate::xml::layer_meta`] and
//! [`crate::xml::layer_meta_write`] that work with raw byte slices instead
//! of typed XML structures, keeping `reader.rs` / `writer.rs` free of
//! quick-xml imports.

use uuid::Uuid;

use crate::{
    error::AifError,
    xml::{
        layer_from_spec, read_layer_meta_xml, write_layer_meta_xml, LayerMetaSpec,
    },
};
use iris_pixel::Layer;

// ── Read ──────────────────────────────────────────────────────────────────────

/// Parse a raw `meta.xml` byte slice for the given layer.
///
/// Returns the [`LayerMetaSpec`] which reader.rs will convert to a
/// [`Layer`] after all tiles have been loaded.
pub(crate) fn parse_layer_meta(
    bytes: &[u8],
    layer_id: Uuid,
) -> Result<LayerMetaSpec, AifError> {
    read_layer_meta_xml(bytes, layer_id)
}

/// Convert a fully-loaded [`LayerMetaSpec`] into a concrete [`Layer`].
pub(crate) fn spec_to_layer(spec: LayerMetaSpec) -> Layer {
    layer_from_spec(spec)
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Serialise a [`Layer`] to a `meta.xml` byte slice.
pub(crate) fn serialise_layer_meta(layer: &Layer) -> Result<Vec<u8>, AifError> {
    write_layer_meta_xml(layer)
}
