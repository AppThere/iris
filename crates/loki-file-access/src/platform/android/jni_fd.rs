// SPDX-License-Identifier: MIT
// Copyright (c) 2026 AppThere

//! Android file-descriptor and permission-checking JNI helpers.
//!
//! Split from [`super::jni_intents`] to keep each file under 300 lines.

use crate::error::{AccessError, PickerError};
use crate::token::PermissionStatus;

/// Query `ContentResolver.getPersistedUriPermissions()` for a URI.
pub(in crate::platform) fn check_persisted_permission(
    uri: &str,
) -> Result<PermissionStatus, PickerError> {
    let vm = super::jni_intents::checked_vm()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(super::jni_intents::attach_err)?;

    let resolver = super::jni_intents::get_content_resolver(&mut env)?;

    let list = env
        .call_method(
            &resolver,
            "getPersistedUriPermissions",
            "()Ljava/util/List;",
            &[],
        )
        .map_err(|e| super::jni_intents::platform_err("getPersistedUriPermissions", e))?
        .l()
        .map_err(|e| super::jni_intents::platform_err("result", e))?;

    let size = env
        .call_method(&list, "size", "()I", &[])
        .map_err(|e| super::jni_intents::platform_err("size", e))?
        .i()
        .map_err(|e| super::jni_intents::platform_err("size int", e))?;

    for i in 0..size {
        let perm = env
            .call_method(
                &list,
                "get",
                "(I)Ljava/lang/Object;",
                &[jni::objects::JValueGen::Int(i)],
            )
            .map_err(|e| super::jni_intents::platform_err("get", e))?
            .l()
            .map_err(|e| super::jni_intents::platform_err("get object", e))?;

        let perm_uri = env
            .call_method(&perm, "getUri", "()Landroid/net/Uri;", &[])
            .map_err(|e| super::jni_intents::platform_err("getUri", e))?
            .l()
            .map_err(|e| super::jni_intents::platform_err("getUri object", e))?;

        let s: String = env
            .call_method(&perm_uri, "toString", "()Ljava/lang/String;", &[])
            .map_err(|e| super::jni_intents::platform_err("toString", e))?
            .l()
            .map_err(|e| super::jni_intents::platform_err("toString object", e))
            .and_then(|obj| {
                env.get_string((&obj).into())
                    .map_err(|e| super::jni_intents::platform_err("read string", e))
            })?
            .into();

        if s == uri {
            return Ok(PermissionStatus::Valid);
        }
    }

    Ok(PermissionStatus::Revoked)
}

/// Open a file descriptor for a content URI via `ContentResolver`.
pub(in crate::platform) fn open_fd(uri: &str, mode: &str) -> Result<i32, AccessError> {
    let vm = super::jni_intents::checked_vm().map_err(|_| access_err("get JavaVM"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|_| access_err("attach thread"))?;

    let uri_obj = parse_uri_for_access(&mut env, uri)?;
    let mode_str = env
        .new_string(mode)
        .map_err(|_| access_err("mode string"))?;

    let activity =
        super::jni_intents::checked_activity().map_err(|_| access_err("get activity"))?;
    let resolver = env
        .call_method(
            &activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .map_err(|_| access_err("getContentResolver"))?
        .l()
        .map_err(|_| AccessError::InvalidDescriptor)?;

    let pfd = env
        .call_method(
            &resolver,
            "openFileDescriptor",
            "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
            &[
                jni::objects::JValueGen::Object(&uri_obj),
                jni::objects::JValueGen::Object(&mode_str),
            ],
        )
        .map_err(|_| access_err("openFileDescriptor"))?
        .l()
        .map_err(|_| AccessError::InvalidDescriptor)?;

    let fd = env
        .call_method(&pfd, "detachFd", "()I", &[])
        .map_err(|_| access_err("detachFd"))?
        .i()
        .map_err(|_| AccessError::InvalidDescriptor)?;
    // A negative descriptor would make the caller's `from_raw_fd` unsound.
    if fd < 0 {
        return Err(AccessError::InvalidDescriptor);
    }
    Ok(fd)
}

/// Parse a URI string for access-error contexts.
fn parse_uri_for_access<'a>(
    env: &mut jni::JNIEnv<'a>,
    uri: &str,
) -> Result<jni::objects::JObject<'a>, AccessError> {
    let cls = env
        .find_class("android/net/Uri")
        .map_err(|_| access_err("Uri class"))?;
    let s = env
        .new_string(uri)
        .map_err(|_| access_err("URI string"))?;
    env.call_static_method(
        &cls,
        "parse",
        "(Ljava/lang/String;)Landroid/net/Uri;",
        &[jni::objects::JValueGen::Object(&s)],
    )
    .map_err(|_| access_err("Uri.parse"))?
    .l()
    .map_err(|_| AccessError::InvalidDescriptor)
}

fn access_err(ctx: &str) -> AccessError {
    AccessError::Platform {
        message: format!("{ctx} failed"),
    }
}
