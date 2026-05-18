// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Typed error enum for all iris-aif read/write operations.

/// All errors that iris-aif can produce.
///
/// Variants are grouped as **Fatal** (unrecoverable; file is unusable) or
/// **Recoverable** (the document may be partially readable).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AifError {
    // ── Fatal ────────────────────────────────────────────────────────────────

    /// Returned when a raw `&Path` is passed on a sandboxed platform (iOS, Android).
    /// Callers must use [`appthere_file_access::FilePicker`] to obtain a
    /// [`appthere_file_access::FileAccessToken`] first.
    #[error(
        "direct path access is not supported on this platform; \
         use FilePicker::pick_file_to_open to obtain a FileAccessToken"
    )]
    PathAccessDenied,

    /// The AIF format major version in `document.xml` is newer than this
    /// library supports and cannot be safely read.
    #[error("unsupported AIF major version {major} (library supports up to {max_supported})")]
    UnsupportedMajorVersion {
        /// The major version found in the file.
        major: u32,
        /// The highest major version this build can read.
        max_supported: u32,
    },

    /// A required OPC part (e.g. `document.xml`) is absent from the archive.
    #[error("required AIF part is missing: {part}")]
    MissingRequiredPart {
        /// The part URI that was expected but not found.
        part: String,
    },

    /// An I/O error occurred while reading from or writing to the file.
    #[error("I/O error: {source}")]
    Io {
        /// The underlying I/O error.
        #[from]
        source: std::io::Error,
    },

    /// The file-access permission was revoked by the OS mid-operation.
    #[error("file access permission revoked during operation")]
    PermissionRevoked,

    /// A file-access error was returned by the platform file-access layer.
    #[error("file access error: {source}")]
    FileAccess {
        /// The underlying access error from appthere-file-access.
        #[from]
        source: appthere_file_access::AccessError,
    },

    // ── Recoverable ──────────────────────────────────────────────────────────

    /// The XML in a part is not well-formed or is missing required attributes.
    // TODO(iris): SPEC.md §4.15 — split into finer-grained variants at milestone 2
    #[error("malformed XML in part {part}: {message}")]
    MalformedXml {
        /// The part URI that contained malformed XML.
        part: String,
        /// Description of the parse failure.
        message: String,
    },

    /// An EXR tile referenced in the layer metadata could not be decoded.
    #[error("EXR decode error for layer {layer_id}: {message}")]
    ExrDecode {
        /// UUID of the layer whose tile failed to decode.
        layer_id: String,
        /// Description of the EXR error.
        message: String,
    },
}
