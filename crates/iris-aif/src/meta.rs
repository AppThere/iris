// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Per-layer `meta.xml` I/O helpers used by `reader.rs` and `writer.rs`.
//!
//! XML parsing and serialisation live in [`crate::xml::layer_meta`] and
//! [`crate::xml::layer_meta_write`]; this module provides the higher-level
//! wrappers that deal with OPC part bytes directly.
//!
//! Wired in PROMPT 3C alongside `reader.rs` and `writer.rs`.

// TODO(iris): SPEC.md §4.6 — meta.rs entry-point functions added in PROMPT 3C.
// The types (LayerMetaSpec, PixelDataSpec, etc.) live in crate::xml::layer_meta.
