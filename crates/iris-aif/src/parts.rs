// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! OPC part URI constants, content-type strings, namespace URIs, and
//! format version constants used throughout iris-aif.
//!
//! **All** OPC string literals used by iris-aif live here exclusively.
//! No other source file may contain raw OPC part URI strings or
//! content-type strings.

// ── Format version ───────────────────────────────────────────────────────────

/// AIF format major version this library reads and writes.
pub const AIF_MAJOR: u32 = 1;

/// AIF format minor version this library reads and writes.
pub const AIF_MINOR: u32 = 0;

/// AIF format version string written into `document.xml`.
pub const AIF_FORMAT_VERSION: &str = "1.0";

// ── XML namespaces ────────────────────────────────────────────────────────────

/// Primary AIF XML namespace.
pub const IRIS_NS: &str = "https://appthere.dev/iris/2024";

/// AIF extension namespace (`x:` prefix). Unknown elements in this namespace
/// are preserved on round-trip and silently ignored on read (§4.1 rule 3).
pub const IRIS_EXT_NS: &str = "https://appthere.dev/iris/ext";

// ── Fixed part URIs ───────────────────────────────────────────────────────────

/// Root document manifest — always the first part parsed.
pub const DOCUMENT_XML: &str = "iris/document.xml";

/// Document metadata (title, author, editing history).
pub const METADATA_XML: &str = "iris/metadata.xml";

/// Loro CRDT op log — opaque binary blob managed by `iris-ops`.
pub const HISTORY_OPS_BIN: &str = "iris/history/ops.bin";

/// sRGB PNG composite preview thumbnail (256 × 256 or smaller).
pub const PREVIEW_PNG: &str = "iris/preview.png";

// ── Dynamic part URI helpers ──────────────────────────────────────────────────

/// Returns the part URI for a layer's `meta.xml`.
///
/// Example: `iris/layers/3f2a1b4c-8d9e-4f0a-b1c2-d3e4f5a6b7c8/meta.xml`
pub fn layer_meta_xml(layer_id: &uuid::Uuid) -> String {
    format!("iris/layers/{layer_id}/meta.xml")
}

/// Returns the part URI for a vector layer's FlatBuffers path store (§4.10).
///
/// Example: `iris/layers/3f2a1b4c-8d9e-4f0a-b1c2-d3e4f5a6b7c8/paths.bin`
pub fn layer_paths_bin(layer_id: &uuid::Uuid) -> String {
    format!("iris/layers/{layer_id}/paths.bin")
}

/// Returns the part URI for a single EXR tile.
///
/// Tile coordinates are zero-padded to six digits per §4.3.
/// Example: `iris/layers/{id}/tiles/000000_000003.exr`
pub fn layer_tile_exr(layer_id: &uuid::Uuid, tx: u32, ty: u32) -> String {
    format!("iris/layers/{layer_id}/tiles/{tx:06}_{ty:06}.exr")
}

// ── Content-type strings (§4.14) ──────────────────────────────────────────────

/// Override content type for `iris/document.xml`.
pub const CT_DOCUMENT: &str = "application/vnd.appthere.iris.document+xml";

/// Override content type for `iris/metadata.xml`.
pub const CT_METADATA: &str = "application/vnd.appthere.iris.metadata+xml";

/// Default content type for `*.bin` op-log and path-store parts.
pub const CT_OPLOG: &str = "application/vnd.appthere.iris.oplog";

/// Default content type for `*.exr` tile parts.
pub const CT_EXR: &str = "image/x-exr";

/// Default content type for binary asset parts (op log, paths.bin).
pub const CT_BIN: &str = "application/octet-stream";

/// Default content type for `preview.png`.
pub const CT_PNG: &str = "image/png";

/// Default content type for embedded ICC profiles.
pub const CT_ICC: &str = "application/vnd.iccprofile";

/// Default content type for brush asset parts.
pub const CT_BRUSH: &str = "application/vnd.appthere.brush+flatbuffers";

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_meta_xml_format() {
        let id = uuid::Uuid::nil();
        assert_eq!(
            layer_meta_xml(&id),
            "iris/layers/00000000-0000-0000-0000-000000000000/meta.xml"
        );
    }

    #[test]
    fn layer_tile_exr_zero_padding() {
        let id = uuid::Uuid::nil();
        assert_eq!(
            layer_tile_exr(&id, 0, 3),
            "iris/layers/00000000-0000-0000-0000-000000000000/tiles/000000_000003.exr"
        );
        assert_eq!(
            layer_tile_exr(&id, 123456, 999999),
            "iris/layers/00000000-0000-0000-0000-000000000000/tiles/123456_999999.exr"
        );
    }

    #[test]
    fn format_version_constant() {
        assert_eq!(AIF_FORMAT_VERSION, "1.0");
        assert_eq!(AIF_MAJOR, 1);
        assert_eq!(AIF_MINOR, 0);
    }
}
