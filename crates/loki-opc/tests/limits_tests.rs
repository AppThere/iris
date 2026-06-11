// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Malformed-container and decompression-limit tests for the ZIP read path.

use std::io::Cursor;

use loki_opc::{OpcError, Package, PartData, PartName, ReadLimits};

/// Unwrap the error side of a `Package` open result (`Package` is not `Debug`).
fn open_err(result: Result<Package, OpcError>, context: &str) -> OpcError {
    match result {
        Ok(_) => panic!("{context}: expected an error but open succeeded"),
        Err(e) => e,
    }
}

/// Build a valid package in memory containing one part of `part_len` bytes.
fn package_bytes_with_part_len(part_len: usize) -> Vec<u8> {
    let mut pkg = Package::new();
    pkg.set_part(
        PartName::new("/word/document.xml").expect("valid part name"),
        // Repetitive content compresses extremely well, like a bomb payload.
        PartData::xml(vec![b'a'; part_len]),
    );
    let mut buffer = Cursor::new(Vec::new());
    pkg.write(&mut buffer, None).expect("write package");
    buffer.into_inner()
}

#[test]
fn garbage_bytes_are_rejected_not_panicking() {
    let garbage = b"this is not a zip archive at all".to_vec();
    let err = open_err(Package::open(Cursor::new(garbage)), "garbage bytes");
    assert!(matches!(err, OpcError::Zip(_)), "expected Zip error, got {err:?}");
}

#[test]
fn truncated_archive_is_rejected_not_panicking() {
    let bytes = package_bytes_with_part_len(4096);
    // Cut the archive in half — central directory is gone.
    let truncated = bytes[..bytes.len() / 2].to_vec();
    assert!(Package::open(Cursor::new(truncated)).is_err());
}

#[test]
fn empty_zip_is_missing_content_types() {
    // A structurally valid but empty ZIP: end-of-central-directory record only.
    let eocd: Vec<u8> = vec![
        0x50, 0x4b, 0x05, 0x06, // EOCD signature
        0, 0, 0, 0, 0, 0, 0, 0, // disk numbers, entry counts
        0, 0, 0, 0, // central directory size
        0, 0, 0, 0, // central directory offset
        0, 0, // comment length
    ];
    let err = open_err(Package::open(Cursor::new(eocd)), "empty zip");
    assert!(
        matches!(err, OpcError::MissingContentTypes),
        "expected MissingContentTypes, got {err:?}"
    );
}

#[test]
fn entry_over_per_part_limit_is_rejected() {
    // 64 KiB part, 1 KiB limit: the read must stop at the cap, not inflate it all.
    let bytes = package_bytes_with_part_len(64 * 1024);
    let limits = ReadLimits { max_part_bytes: 1024, max_total_bytes: u64::MAX };
    let err = open_err(
        Package::open_with_limits(Cursor::new(bytes), &limits),
        "per-part limit",
    );
    assert!(
        matches!(err, OpcError::EntryTooLarge { limit: 1024, .. }),
        "expected EntryTooLarge, got {err:?}"
    );
}

#[test]
fn entry_at_per_part_limit_is_accepted() {
    let bytes = package_bytes_with_part_len(1024);
    // Limit must cover the document part and the OPC structural parts.
    let limits = ReadLimits { max_part_bytes: 4096, max_total_bytes: u64::MAX };
    let pkg = Package::open_with_limits(Cursor::new(bytes), &limits).expect("within limits");
    assert!(pkg
        .part(&PartName::new("/word/document.xml").expect("name"))
        .is_some());
}

#[test]
fn package_over_total_limit_is_rejected() {
    let bytes = package_bytes_with_part_len(8 * 1024);
    // Each entry fits individually, but the running total cannot.
    let limits = ReadLimits { max_part_bytes: u64::MAX, max_total_bytes: 8 * 1024 };
    let err = open_err(
        Package::open_with_limits(Cursor::new(bytes), &limits),
        "total limit",
    );
    assert!(
        matches!(err, OpcError::PackageTooLarge { limit } if limit == 8 * 1024),
        "expected PackageTooLarge, got {err:?}"
    );
}

#[test]
fn default_limits_accept_normal_packages() {
    let bytes = package_bytes_with_part_len(64 * 1024);
    assert!(Package::open(Cursor::new(bytes)).is_ok());
}
