// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel tool implementations for AppThere Iris.
//!
//! Phase 3 scope: hard round brush and eraser. Stylus pressure scales size.
//! Phase 4: texture, scatter, wet paint, clone stamp, heal.

#![forbid(unsafe_code)]

pub mod brush;
pub mod eraser;
pub mod eyedropper;
pub mod fill;

pub use brush::{BrushEngine, BrushSettings};
pub use eraser::EraserEngine;
pub use eyedropper::sample_color;
pub use fill::flood_fill;
