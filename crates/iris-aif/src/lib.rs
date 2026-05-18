// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Artisan Interchange Format (.aif) read/write
//!
//! See SPEC.md and crates/iris-aif/BRIEF.md before implementing.
//!
//! # File access
//!
//! All file I/O is performed via [`appthere_file_access`].  Callers should
//! obtain a [`FileAccessToken`] from [`FilePicker`] and pass it to
//! `AifReader::open_token` / `AifWriter::write_token`.  On desktop a
//! convenience `&Path` API is also available; it returns
//! [`AifError::PathAccessDenied`] on sandboxed platforms (iOS, Android).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;

// Re-export AifError at the crate root.
pub use error::AifError;

// Re-export the file-access surface so iris-app callers never need to import
// appthere-file-access directly.
pub use appthere_file_access::{
    AccessError, FileAccessToken, FilePicker, PickOptions, PickerError, SaveOptions,
};

// TODO(iris): SPEC.md §4 — AifReader, AifWriter, AifDocument stubs follow in milestone 1
