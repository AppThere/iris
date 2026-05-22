// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Typed error enum for all iris-aif read/write operations.
//!
//! Errors are grouped as **Fatal** (the document is unreadable and the
//! operation must be aborted) or **Recoverable** (the document can be
//! opened in a degraded state with a visible warning to the user).
//!
//! See SPEC.md §4.15 for the normative error taxonomy.

use appthere_file_access::AccessError;

/// All errors that iris-aif read/write operations can produce.
///
/// Match exhaustively where possible; `#[non_exhaustive]` is present because
/// this crate may be published externally and new variants may be added in
/// minor releases without breaking semver.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AifError {
    // ── Fatal errors ─────────────────────────────────────────────────────────

    /// Direct `&Path` access is not supported on sandboxed platforms (iOS,
    /// Android). Callers must use [`appthere_file_access::FilePicker`] to
    /// obtain a [`appthere_file_access::FileAccessToken`] first.
    #[error(
        "direct path access is not supported on this platform; \
         use FilePicker::pick_file_to_open to obtain a FileAccessToken"
    )]
    PathAccessDenied,

    /// The AIF format major version in `document.xml` is newer than this
    /// library supports.
    #[error(
        "unsupported AIF major version {found}; \
         this reader supports up to major version {supported}"
    )]
    UnsupportedMajorVersion {
        /// The major version found in the file.
        found: u32,
        /// The highest major version this build can read.
        supported: u32,
    },

    /// A required OPC part is absent from the archive.
    #[error("required AIF part is missing: {0}")]
    MissingRequiredPart(String),

    /// A required XML attribute was absent on an element.
    #[error("document.xml: missing required attribute '{attr}' on <{element}>")]
    MissingAttribute {
        /// The XML element name (without namespace prefix).
        element: String,
        /// The attribute name that was expected but absent.
        attr: String,
    },

    /// A pixel-mode document has an artboard that does not match the canvas
    /// bounds, or has more or fewer than one artboard.
    #[error(
        "pixel-mode document must have exactly one artboard \
         whose bounds equal the canvas dimensions"
    )]
    InvalidPixelModeArtboard,

    /// Two or more layers in the layer tree share the same UUID.
    #[error("duplicate layer id {0} in layer tree")]
    DuplicateLayerId(uuid::Uuid),

    /// A layer declared in `document.xml` has no corresponding `meta.xml` part.
    #[error("layer {layer_id} declared in document.xml but meta.xml is missing")]
    MissingLayerMeta {
        /// The layer UUID that has no `meta.xml`.
        layer_id: uuid::Uuid,
    },

    /// A layer's `meta.xml` contains an unrecognised blend mode string.
    /// Silently substituting `normal` would change document appearance without
    /// user awareness; this is always fatal (SPEC.md §4.8).
    #[error("layer {layer_id}: unknown blend mode '{value}'")]
    UnknownBlendMode {
        /// The layer UUID whose blend mode could not be decoded.
        layer_id: uuid::Uuid,
        /// The unrecognised blend mode string as read from `meta.xml`.
        value: String,
    },

    /// A layer's `meta.xml` contains an unrecognised colour space identifier.
    #[error("layer {layer_id}: unknown colour space '{value}'")]
    UnknownColorSpace {
        /// The layer UUID whose colour space could not be decoded.
        layer_id: uuid::Uuid,
        /// The unrecognised identifier string.
        value: String,
    },

    /// The `colorSpace` in a layer's `meta.xml` does not match the
    /// `PathStore.colorSpace` in its `paths.bin` (Phase 3 path; §4.10).
    #[error("layer {layer_id}: colour space mismatch between meta.xml and paths.bin")]
    ColorSpaceMismatch {
        /// The layer UUID with the mismatch.
        layer_id: uuid::Uuid,
    },

    /// The `aif:layerId`, `aif:tileX`, or `aif:tileY` attributes embedded
    /// in an EXR tile do not match the OPC part path.
    #[error(
        "layer {layer_id} tile ({tx},{ty}): \
         EXR metadata mismatch (expected layer id {expected_id})"
    )]
    TileMetadataMismatch {
        /// The layer UUID derived from the OPC part path.
        layer_id: uuid::Uuid,
        /// Tile X coordinate from the part path.
        tx: u32,
        /// Tile Y coordinate from the part path.
        ty: u32,
        /// The UUID embedded in the EXR `aif:layerId` attribute.
        expected_id: uuid::Uuid,
    },

    /// A vector layer's `paths.bin` carries a `PathStore.version` greater
    /// than `1` (the only version this library understands).
    /// Phase 3 path; included per §4.15.
    #[error("layer {layer_id}: unknown PathStore version {version}")]
    UnknownPathStoreVersion {
        /// The layer UUID.
        layer_id: uuid::Uuid,
        /// The version number found in the FlatBuffers header.
        version: u32,
    },

    /// A `PathData` in `paths.bin` has a mismatch between verb count and
    /// point array length.
    /// Phase 3 path; included per §4.15.
    #[error("layer {layer_id}: malformed path data (verb/point count mismatch)")]
    MalformedPathData {
        /// The layer UUID.
        layer_id: uuid::Uuid,
    },

    /// A `PathObject.transform` array has a length other than 6.
    /// Phase 3 path; included per §4.15.
    #[error(
        "layer {layer_id}: malformed transform \
         (expected 6 floats, found {got})"
    )]
    MalformedTransform {
        /// The layer UUID.
        layer_id: uuid::Uuid,
        /// The actual number of floats found.
        got: usize,
    },

    /// An XML part is malformed or fails schema validation.
    ///
    /// // AUDIT: SPEC.md §4.15 names this `XmlParse` with a `Box<dyn Error>`
    /// // source field. CLAUDE.md §"Error handling" forbids `Box<dyn Error>` at
    /// // public API boundaries. Using a `String` message field instead, consistent
    /// // with `ExrEncode` and `ExrDecode`.
    #[error("XML parse error in '{part}': {message}")]
    XmlParse {
        /// The OPC part URI where the parse error occurred.
        part: String,
        /// Human-readable description of the failure.
        message: String,
    },

    /// EXR encoding failed while writing a pixel tile.
    #[error(
        "EXR encode error writing layer {layer_id} tile ({tx},{ty}): {message}"
    )]
    ExrEncode {
        /// UUID of the layer whose tile failed to encode.
        layer_id: uuid::Uuid,
        /// Tile column index.
        tx: u32,
        /// Tile row index.
        ty: u32,
        /// Description of the EXR error.
        message: String,
    },

    /// Fatal OPC container error (malformed ZIP, missing content types, etc.).
    #[error("OPC container error: {0}")]
    Opc(#[from] loki_opc::OpcError),

    /// Fatal I/O error while reading from or writing to the file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The file-access layer returned an error (e.g. permission denied by OS).
    #[error("file access error: {source}")]
    FileAccess {
        /// The underlying platform error.
        source: AccessError,
    },

    /// The OS revoked the file-access permission mid-operation (e.g. the user
    /// deselected the file in a security dialog on iOS).
    #[error("file access permission was revoked by the OS during the operation")]
    PermissionRevoked,

    /// Failed to import a raster image (e.g. invalid format or decoding failure).
    #[error("failed to import raster image: {0}")]
    ImportError(String),

    // ── Recoverable errors ───────────────────────────────────────────────────

    /// An EXR tile could not be decoded; the compositor substitutes a fully
    /// transparent tile and continues loading the document.
    ///
    /// // AUDIT: SPEC.md §4.15 names the source field `Box<dyn Error + Send + Sync>`.
    /// // CLAUDE.md forbids Box<dyn Error> at public API boundaries.
    /// // Using String message field instead.
    #[error("EXR decode error for layer {layer_id} tile ({tx},{ty}): {message}")]
    TileReadError {
        /// The layer UUID.
        layer_id: uuid::Uuid,
        /// Tile column index.
        tx: u32,
        /// Tile row index.
        ty: u32,
        /// Description of the decode failure.
        message: String,
    },

    /// The op log magic header or version is invalid; the document is opened
    /// without editing history. Pixel and vector data are unaffected.
    #[error("op log invalid or unreadable (document opened without history): {reason}")]
    InvalidOpLog {
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// An EXR tile was found but could not be decoded (legacy variant, kept
    /// for compatibility with error paths written before `TileReadError`).
    #[error("EXR decode error for layer {layer_id}: {message}")]
    ExrDecode {
        /// The layer UUID.
        layer_id: uuid::Uuid,
        /// Description of the failure.
        message: String,
    },
}

impl From<AccessError> for AifError {
    fn from(e: AccessError) -> Self {
        AifError::FileAccess { source: e }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_major_version_message() {
        let e = AifError::UnsupportedMajorVersion { found: 99, supported: 1 };
        let msg = e.to_string();
        assert!(msg.contains("99"), "message must include found version");
        assert!(msg.contains('1'), "message must include supported version");
    }

    #[test]
    fn missing_required_part_message() {
        let e = AifError::MissingRequiredPart("iris/document.xml".into());
        assert!(e.to_string().contains("iris/document.xml"));
    }

    #[test]
    fn from_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let aif: AifError = io.into();
        assert!(matches!(aif, AifError::Io(_)));
    }
}
