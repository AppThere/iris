// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Artisan Interchange Format (`.aif`) read/write.
//!
//! An AIF file is an OPC/ZIP container (managed by `loki-opc`) containing:
//! - `iris/document.xml` — root manifest and layer tree
//! - `iris/metadata.xml` — document title, author, editing sessions
//! - `iris/layers/{id}/meta.xml` — per-layer properties
//! - `iris/layers/{id}/tiles/{tx}_{ty}.exr` — pixel tile data (f16 RGBA EXR)
//! - `iris/history/ops.bin` — Loro CRDT op log (Phase 2)
//! - `iris/preview.png` — 256×256 sRGB composite thumbnail
//!
//! See SPEC.md §4 and `crates/iris-aif/BRIEF.md` for the full specification.
//!
//! # File access
//!
//! All file I/O is performed via [`appthere_file_access`]. Callers obtain a
//! [`FileAccessToken`] from [`FilePicker`] and pass it to
//! `AifReader::open_token` / `AifWriter::write_token`. On desktop a
//! convenience `&Path` API is also available; it returns
//! [`AifError::PathAccessDenied`] on sandboxed platforms (iOS, Android).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod document;
pub mod error;
pub mod parts;
pub(crate) mod xml;
pub(crate) mod meta;
// Modules added in PROMPT 3C:
// pub(crate) mod tile;
// pub(crate) mod preview;
// pub mod reader;
// pub mod writer;

// ── Primary type re-exports ───────────────────────────────────────────────────

pub use document::{AifArtboard, AifCanvas, AifDocument, CanvasMode};
pub use error::AifError;

// ── OPC surface re-export ─────────────────────────────────────────────────────

/// Re-export the ZIP compression selector so callers need not import loki-opc.
pub use loki_opc::CompressionMethod;

// ── File-access surface re-export ─────────────────────────────────────────────

/// Re-export file-access types so iris-app never imports appthere-file-access.
pub use appthere_file_access::{
    AccessError, FileAccessToken, FilePicker, PickOptions, PickerError, SaveOptions,
};

// TODO(iris): SPEC.md §4 — AifReader and AifWriter re-exported in PROMPT 3C
