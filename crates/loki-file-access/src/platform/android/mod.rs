// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Android file-picker implementation using the Storage Access Framework (SAF).
//!
//! This module uses JNI to interact with Android's `Intent.ACTION_OPEN_DOCUMENT`
//! and `Intent.ACTION_CREATE_DOCUMENT` intents.  File access is mediated
//! through content URIs and `ContentResolver`, ensuring that the app never
//! accesses files via filesystem paths (which are unreliable on modern Android).
//!
//! # Persistence
//!
//! After the user selects a file, this module calls
//! `ContentResolver.takePersistableUriPermission()` with both READ and WRITE
//! flags.  This ensures that the URI grant survives app restarts and device
//! reboots, allowing the application to maintain a reliable recent-files list.
//!
//! # Integration
//!
//! The host Android activity must forward `onActivityResult` to
//! [`on_activity_result`] so that the pending future can be resolved.

mod jni_fd;
mod jni_intents;

use std::sync::{Arc, Mutex, OnceLock};

use crate::api::{PickOptions, SaveOptions};
use crate::error::{AccessError, PickerError};
use crate::future::{deliver, new_pick_future};
use crate::token::{
    FileAccessToken, PermissionStatus, ReadSeek, TokenInner, WriteSeek,
};

/// Pending pick state shared between the intent launcher and the JNI callback.
static PENDING_PICK: OnceLock<
    Mutex<Option<Arc<Mutex<crate::future::PickState<Option<String>>>>>>,
> = OnceLock::new();

fn pending_pick(
) -> &'static Mutex<Option<Arc<Mutex<crate::future::PickState<Option<String>>>>>> {
    PENDING_PICK.get_or_init(|| Mutex::new(None))
}

/// Pick a single file for reading via `ACTION_OPEN_DOCUMENT`.
pub(crate) async fn pick_open_single(
    options: PickOptions,
) -> Result<Option<FileAccessToken>, PickerError> {
    let uri = launch_open_intent(&options, false).await?;
    match uri {
        None => Ok(None),
        Some(uri_str) => {
            jni_intents::take_persistable_uri_permission(&uri_str)?;
            let display_name =
                uri_str.rsplit('/').next().unwrap_or("unnamed").to_owned();
            Ok(Some(FileAccessToken {
                inner: TokenInner::Android {
                    uri: uri_str,
                    display_name,
                    mime_type: None,
                },
            }))
        }
    }
}

/// Pick multiple files for reading via `ACTION_OPEN_DOCUMENT`.
pub(crate) async fn pick_open_multi(
    options: PickOptions,
) -> Result<Vec<FileAccessToken>, PickerError> {
    let uri = launch_open_intent(&options, true).await?;
    match uri {
        None => Ok(vec![]),
        Some(uri_str) => {
            jni_intents::take_persistable_uri_permission(&uri_str)?;
            let display_name =
                uri_str.rsplit('/').next().unwrap_or("unnamed").to_owned();
            Ok(vec![FileAccessToken {
                inner: TokenInner::Android {
                    uri: uri_str,
                    display_name,
                    mime_type: None,
                },
            }])
        }
    }
}

/// Pick a save location via `ACTION_CREATE_DOCUMENT`.
pub(crate) async fn pick_save(
    options: SaveOptions,
) -> Result<Option<FileAccessToken>, PickerError> {
    let uri = launch_create_intent(&options).await?;
    match uri {
        None => Ok(None),
        Some(uri_str) => {
            jni_intents::take_persistable_uri_permission(&uri_str)?;
            let display_name = options
                .suggested_name
                .clone()
                .unwrap_or_else(|| "untitled".into());
            Ok(Some(FileAccessToken {
                inner: TokenInner::Android {
                    uri: uri_str,
                    display_name,
                    mime_type: options.mime_type.clone(),
                },
            }))
        }
    }
}

/// Open a content URI for reading.
pub(crate) fn open_read(inner: &TokenInner) -> Result<Box<dyn ReadSeek>, AccessError> {
    match inner {
        TokenInner::Android { uri, .. } => {
            let fd = jni_fd::open_fd(uri, "r")?;
            // SAFETY: `open_fd` returns a valid file descriptor from
            // Android's `ContentResolver.openFileDescriptor` after detaching
            // it.  The caller takes ownership; it must not be double-closed.
            let file = unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) };
            Ok(Box::new(file))
        }
        _ => Err(AccessError::Platform {
            message: "non-Android token on Android platform".into(),
        }),
    }
}

/// Open a content URI for writing.
pub(crate) fn open_write(inner: &TokenInner) -> Result<Box<dyn WriteSeek>, AccessError> {
    match inner {
        TokenInner::Android { uri, .. } => {
            let fd = jni_fd::open_fd(uri, "w")?;
            // SAFETY: Same invariant as `open_read` — see above.
            let file: std::fs::File =
                unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) };
            Ok(Box::new(file))
        }
        _ => Err(AccessError::Platform {
            message: "non-Android token on Android platform".into(),
        }),
    }
}

/// Open a content URI for writing, truncating to zero length before returning.
///
/// # Platform note
// COMPAT(mobile): Android "w" mode truncates implicitly — ContentResolver
// openFileDescriptor with mode "w" replaces content, so this delegates to open_write.
pub(crate) fn open_write_truncate(inner: &TokenInner) -> Result<Box<dyn WriteSeek>, AccessError> {
    open_write(inner)
}

/// Check whether a persistable URI permission is still held.
pub(crate) fn check_permission(inner: &TokenInner) -> PermissionStatus {
    match inner {
        TokenInner::Android { uri, .. } => jni_fd::check_persisted_permission(uri)
            .unwrap_or(PermissionStatus::Unknown),
        _ => PermissionStatus::Unknown,
    }
}

/// Called from Java's `onActivityResult` via JNI to deliver the selected
/// URI (or `None` if the user cancelled) and wake the pending future.
pub fn on_activity_result(uri: Option<String>) {
    let guard = match pending_pick().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(ref state) = *guard {
        deliver(state, uri);
    }
}

/// Deliver multiple URI results from Android `onActivityResult` to a pending
/// `pick_files_to_open` future.  Call this from the JNI bridge when
/// `EXTRA_ALLOW_MULTIPLE` was set on the Intent.
///
/// The existing [`on_activity_result`] is unchanged and continues to serve
/// single-file results for Loki compatibility.
///
/// # Current limitation
///
/// Full multi-file support requires changing the `PENDING_PICK` state type to
/// `Vec<String>`, which is a breaking change for existing Loki JNI bridges.
// TODO(iris): loki-file-access Android — full multi-file PENDING_PICK state
// change tracked in gap list; see PROMPT 2C §6 Android item 1.
pub fn on_activity_result_multi(uris: Vec<String>) {
    // Deliver only the first URI to avoid silent data loss.
    // A full implementation requires PENDING_PICK to hold Vec<String>.
    let first = uris.into_iter().next();
    on_activity_result(first);
}

/// Store the shared state for the in-flight pick operation.
fn store_pending(
    state: Arc<Mutex<crate::future::PickState<Option<String>>>>,
) -> Result<(), PickerError> {
    let mut guard = pending_pick().lock().map_err(|e| PickerError::Internal {
        message: e.to_string(),
    })?;
    // SAFETY(concurrency): guard against concurrent pick invocations.
    if let Some(ref existing) = *guard {
        let in_flight = existing.lock().map(|s| s.result.is_none()).unwrap_or(false);
        if in_flight {
            return Err(PickerError::Internal {
                message: "a file pick operation is already in progress; await the previous pick before starting a new one".into(),
            });
        }
    }
    *guard = Some(state);
    Ok(())
}

/// Clear the pending pick state after a pick operation completes.
fn clear_pending() {
    if let Ok(mut guard) = pending_pick().lock() {
        *guard = None;
    }
}

/// Launch `ACTION_OPEN_DOCUMENT` and await the result.
async fn launch_open_intent(
    options: &PickOptions,
    allow_multiple: bool,
) -> Result<Option<String>, PickerError> {
    let (future, state) = new_pick_future::<Option<String>>();
    store_pending(state)?;
    jni_intents::fire_open_document_intent(options, allow_multiple)?;
    let result = future.await;
    clear_pending();
    Ok(result)
}

/// Launch `ACTION_CREATE_DOCUMENT` and await the result.
async fn launch_create_intent(
    options: &SaveOptions,
) -> Result<Option<String>, PickerError> {
    let (future, state) = new_pick_future::<Option<String>>();
    store_pending(state)?;
    jni_intents::fire_create_document_intent(options)?;
    let result = future.await;
    clear_pending();
    Ok(result)
}
