// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Capability token for accessing user-selected files.
//!
//! [`FileAccessToken`] is the central type returned by every picker operation.
//! It encapsulates all platform-specific state needed to re-open a file,
//! including Android URIs, iOS security-scoped bookmarks, desktop paths, and
//! in-memory WASM data.
//!
//! Tokens are serializable to a URL-safe base64-encoded JSON string via
//! [`FileAccessToken::serialize`] and [`FileAccessToken::deserialize`], making
//! them suitable for persisting in a recent-files list or application database.

mod serde_impl;

use std::path::PathBuf;

use crate::error::AccessError;

/// Status of the permission grant associated with a [`FileAccessToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PermissionStatus {
    /// The token's permission is still valid and the file can be opened.
    Valid,
    /// The permission has been revoked by the user or the operating system.
    Revoked,
    /// The permission status cannot be determined on this platform.
    Unknown,
}

/// Internal representation of platform-specific token data.
///
/// This enum is serialized to JSON and then base64-encoded for storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum TokenInner {
    /// Desktop file identified by filesystem path.
    Desktop {
        /// Absolute path to the file.
        path: PathBuf,
        /// User-visible file name.
        display_name: String,
    },
    /// Android file identified by a content URI.
    Android {
        /// Content URI string (e.g. `content://...`).
        uri: String,
        /// User-visible file name from the document provider.
        display_name: String,
        /// MIME type reported by the document provider.
        mime_type: Option<String>,
    },
    /// iOS file identified by a security-scoped bookmark.
    Ios {
        /// Opaque bookmark data created by `NSURL.bookmarkData(...)`.
        bookmark: Vec<u8>,
        /// User-visible file name.
        display_name: String,
        /// MIME type (often inferred from the file extension).
        mime_type: Option<String>,
    },
    /// WASM file held entirely in memory.
    Wasm {
        /// Complete file contents.
        data: Vec<u8>,
        /// Original file name from the `<input>` element.
        name: String,
        /// MIME type reported by the browser.
        mime_type: Option<String>,
    },
}

/// A serializable capability token representing access to a user-selected file.
///
/// Obtain instances from [`crate::FilePicker`] methods.  Serialize via
/// [`serialize`](Self::serialize) for storage; deserialize to reopen files
/// across app restarts.
#[derive(Debug, Clone)]
pub struct FileAccessToken {
    pub(crate) inner: TokenInner,
}

impl FileAccessToken {
    /// Open the file for reading.  Returns `Read + Seek`.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] if permission is revoked or the file cannot be opened.
    #[must_use = "this returns a Result that may contain an error"]
    pub fn open_read(&self) -> Result<Box<dyn ReadSeek>, AccessError> {
        crate::platform::open_read(&self.inner)
    }

    /// Open the file for writing.  Returns `Write + Seek`.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] if permission is revoked or the file cannot be opened.
    #[must_use = "this returns a Result that may contain an error"]
    pub fn open_write(&self) -> Result<Box<dyn WriteSeek>, AccessError> {
        crate::platform::open_write(&self.inner)
    }

    /// Open the file for writing and truncate to zero length before returning.
    /// Prefer this over `open_write` when creating or fully overwriting a file.
    #[must_use = "this returns a Result that may contain an error"]
    pub fn open_write_truncate(&self) -> Result<Box<dyn WriteSeek>, AccessError> {
        crate::platform::open_write_truncate(&self.inner)
    }

    /// Returns the user-visible display name of the file (typically the filename).
    #[must_use]
    pub fn display_name(&self) -> &str {
        match &self.inner {
            TokenInner::Desktop { display_name, .. }
            | TokenInner::Android { display_name, .. }
            | TokenInner::Ios { display_name, .. } => display_name,
            TokenInner::Wasm { name, .. } => name,
        }
    }

    /// Returns the MIME type of the file, if known.  Desktop returns `None`.
    #[must_use]
    pub fn mime_type(&self) -> Option<&str> {
        match &self.inner {
            TokenInner::Desktop { .. } => None,
            TokenInner::Android { mime_type, .. }
            | TokenInner::Ios { mime_type, .. }
            | TokenInner::Wasm { mime_type, .. } => mime_type.as_deref(),
        }
    }

    /// Check whether the permission grant for this file is still valid.
    #[must_use]
    pub fn check_permission(&self) -> PermissionStatus {
        crate::platform::check_permission(&self.inner)
    }
}

/// Trait object combining [`std::io::Read`] and [`std::io::Seek`].
pub trait ReadSeek: std::io::Read + std::io::Seek + Send {}
impl<T: std::io::Read + std::io::Seek + Send> ReadSeek for T {}

/// Trait object combining [`std::io::Write`] and [`std::io::Seek`].
pub trait WriteSeek: std::io::Write + std::io::Seek + Send {}
impl<T: std::io::Write + std::io::Seek + Send> WriteSeek for T {}
