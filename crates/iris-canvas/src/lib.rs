// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0
//
//! Iris-specific canvas: infinite viewport, tile compositor, blend modes.
//! Built on appthere-canvas generic GPU infrastructure.
//!
//! See SPEC.md §6.2 and ADR/006-shared-canvas-extraction.md.

#![forbid(unsafe_code)]

// Re-export generic canvas types Iris callers need
pub use appthere_canvas::{
    CacheKey, CacheTier, ScrollPhase, ScrollState,
    GpuTexture, PageSource, RenderError,
};

// iris-canvas uses TileCoord as its concrete CacheKey.
// TileCoord: struct TileCoord { pub tx: u32, pub ty: u32 }
// Already derives Hash, Eq, Copy — CacheKey blanket impl covers it.
use iris_pixel::TileCoord;

// Compile-time assertion: TileCoord satisfies CacheKey
const _: () = {
    fn _assert<T: appthere_canvas::CacheKey>() {}
    fn _check() { _assert::<TileCoord>(); }
};

// TODO(iris): SPEC.md §6.2 — Phase 2: CanvasViewport (pan/zoom/rotation)
// pub mod viewport;

// TODO(iris): SPEC.md §6.2 — Phase 2: pixel tile compositor, 27 blend modes
// pub mod compositor;

// TODO(iris): SPEC.md §6.2 — Phase 2: Dioxus IrisCanvas component
// pub mod canvas_widget;

// TODO(iris): SPEC.md §6.2 — Phase 2: overlay pass (selection, guides, artboards)
// pub mod overlay;
