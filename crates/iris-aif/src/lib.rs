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
//! [`reader::AifReader::open_token`] / [`writer::AifWriter::write_token`].
//! On desktop a convenience `Read + Seek` API is also available.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod document;
pub mod error;
pub mod parts;
pub(crate) mod xml;
pub(crate) mod meta;
pub(crate) mod tile;
pub(crate) mod preview;
pub mod reader;
pub mod writer;
mod import;
mod export;

// ── Primary type re-exports ───────────────────────────────────────────────────

pub use document::{AifArtboard, AifCanvas, AifDocument, CanvasMode};
pub use error::AifError;
pub use reader::AifReader;
pub use writer::{AifWriter, WriteOptions};
pub use import::{import_raster_image, layer_from_rgba8};
pub use export::{
    encode_png_rgba8, flatten_to_rgba8, layer_to_rgba8, linear_to_srgb, LayerPixels,
};

// ── OPC surface re-export ─────────────────────────────────────────────────────

/// Re-export the ZIP compression selector so callers need not import loki-opc.
pub use loki_opc::CompressionMethod;

// ── File-access surface re-export ─────────────────────────────────────────────

/// Re-export file-access types so iris-app never imports appthere-file-access.
pub use appthere_file_access::{
    AccessError, FileAccessToken, FilePicker, PickOptions, PickerError, SaveOptions,
};
