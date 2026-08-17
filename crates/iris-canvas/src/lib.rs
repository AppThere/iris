// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0
//
//! Iris-specific canvas: infinite viewport, tile compositor, blend modes.
//! Built on appthere-canvas generic GPU infrastructure.
//!
//! See SPEC.md §6.2 and ADR/006-shared-canvas-extraction.md.

#![forbid(unsafe_code)]

pub mod canvas_widget;
pub mod key;
pub mod tool_event;
pub(crate) mod compositor;
pub(crate) mod paint_bridge;
pub mod viewport;
pub(crate) mod vector;

// TODO(iris): SPEC.md §6.2 — Phase 2: overlay pass (selection, guides, artboards)
// pub mod overlay;

// Re-export generic canvas types Iris callers need
pub use appthere_canvas::{
    CacheKey, CacheTier, GpuTexture, PageSource, RenderError, ScrollPhase, ScrollState,
};

// Iris-specific public API
pub use canvas_widget::IrisCanvas;
pub use compositor::CompositorError;
pub use key::TileKey;
pub use tool_event::{PointerButton, ToolEvent};
pub use viewport::{CanvasViewport, MAX_ZOOM, MIN_ZOOM};
