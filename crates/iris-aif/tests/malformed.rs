// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Malformed-fixture tests: every reachable `AifError` variant is triggered
//! from a corrupt `.aif` file in `tests/fixtures/` (CLAUDE.md test mandate).
//!
//! Regenerate the corpus with:
//! `cargo test -p iris-aif --features gen-fixtures --test gen_fixtures`
//!
//! Variants covered elsewhere:
//! - `TileMetadataMismatch`, `MissingRequiredPart`, `UnsupportedMajorVersion`
//!   — also covered programmatically in `tests/integration.rs`.
//! - `ImportError` — covered in `src/import/tests.rs`.
//! - `Io` — covered in `src/error.rs` unit tests.
//!
//! Variants that no code path constructs yet (no fixture can trigger them);
//! `unreachable_variants_format` below keeps their Display output covered:
//! - `PathAccessDenied`, `PermissionRevoked`, `FileAccess` — platform-gated;
//!   TODO(iris): SPEC.md §10 — fixture tests once loki-file-access mock exists.
//! - `UnknownColorSpace`, `ColorSpaceMismatch`, `UnknownPathStoreVersion`,
//!   `MalformedPathData`, `MalformedTransform` — Phase 3 paths.bin;
//!   TODO(iris): SPEC.md §4.10 — fixtures when the vector read path lands.
//! - `InvalidOpLog` — Phase 2 op log; TODO(iris): SPEC.md §4.11.
//! - `ExrEncode` — write-side; cannot be triggered by reading a fixture.
//! - `ExrDecode` — legacy variant superseded by `TileReadError`.

use std::io::Cursor;

use iris_aif::{AifError, AifReader};

fn open_fixture(bytes: &'static [u8]) -> Result<iris_aif::AifDocument, AifError> {
    AifReader::open(Cursor::new(bytes))
}

macro_rules! fixture_rejects {
    ($test:ident, $file:literal, $pattern:pat) => {
        #[test]
        fn $test() {
            let err = open_fixture(include_bytes!(concat!("fixtures/", $file)))
                .expect_err(concat!($file, " must be rejected"));
            assert!(
                matches!(err, $pattern),
                concat!($file, ": unexpected error variant: {:?}"),
                err
            );
        }
    };
}

fixture_rejects!(not_a_zip, "not_a_zip.aif", AifError::Opc(_));

fixture_rejects!(
    missing_document_xml,
    "missing_document_xml.aif",
    AifError::MissingRequiredPart(_)
);

fixture_rejects!(
    unsupported_major_version,
    "unsupported_major_version.aif",
    AifError::UnsupportedMajorVersion { found: 99, .. }
);

fixture_rejects!(
    malformed_document_xml,
    "malformed_document_xml.aif",
    AifError::XmlParse { .. }
);

fixture_rejects!(missing_canvas, "missing_canvas.aif", AifError::MissingAttribute { .. });

fixture_rejects!(oversized_canvas, "oversized_canvas.aif", AifError::XmlParse { .. });

fixture_rejects!(
    pixel_mode_no_artboard,
    "pixel_mode_no_artboard.aif",
    AifError::InvalidPixelModeArtboard
);

fixture_rejects!(
    duplicate_layer_id,
    "duplicate_layer_id.aif",
    AifError::DuplicateLayerId(_)
);

fixture_rejects!(
    missing_layer_meta,
    "missing_layer_meta.aif",
    AifError::MissingLayerMeta { .. }
);

fixture_rejects!(
    unknown_blend_mode,
    "unknown_blend_mode.aif",
    AifError::UnknownBlendMode { .. }
);

fixture_rejects!(
    oversized_crop_bounds,
    "oversized_crop_bounds.aif",
    AifError::XmlParse { .. }
);

fixture_rejects!(corrupt_tile, "corrupt_tile.aif", AifError::TileReadError { .. });

// Regression: a structurally valid EXR declaring 512×512 previously drove
// out-of-bounds writes into the fixed 256×256 tile buffer (panic = DoS).
fixture_rejects!(oversized_tile, "oversized_tile.aif", AifError::TileReadError { .. });

/// The oversized-canvas error message must say which limit was exceeded.
#[test]
fn oversized_canvas_names_the_limit() {
    let err = open_fixture(include_bytes!("fixtures/oversized_canvas.aif"))
        .expect_err("must be rejected");
    assert!(
        err.to_string().contains(&iris_aif::MAX_CANVAS_DIMENSION.to_string()),
        "error should name the dimension limit: {err}"
    );
}

/// Display coverage for variants no code path constructs yet (see module docs
/// for the per-variant fixture TODOs).
#[test]
fn unreachable_variants_format() {
    let id = uuid::Uuid::nil();
    let cases: Vec<(AifError, &str)> = vec![
        (AifError::PathAccessDenied, "FilePicker"),
        (AifError::PermissionRevoked, "revoked"),
        (AifError::UnknownColorSpace { layer_id: id, value: "p3?".into() }, "p3?"),
        (AifError::ColorSpaceMismatch { layer_id: id }, "mismatch"),
        (AifError::UnknownPathStoreVersion { layer_id: id, version: 9 }, "9"),
        (AifError::MalformedPathData { layer_id: id }, "path data"),
        (AifError::MalformedTransform { layer_id: id, got: 5 }, "5"),
        (AifError::InvalidOpLog { reason: "bad magic".into() }, "bad magic"),
        (
            AifError::ExrEncode { layer_id: id, tx: 0, ty: 0, message: "disk full".into() },
            "disk full",
        ),
        (AifError::ExrDecode { layer_id: id, message: "legacy".into() }, "legacy"),
    ];
    for (err, needle) in cases {
        assert!(
            err.to_string().contains(needle),
            "Display for {err:?} should contain {needle:?}"
        );
    }
}
