// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! CPU rasterisation of vector layers into the compositor's accumulation buffer
//! (SPEC.md §6.2, Phase 3).
//!
//! The live composite path cannot submit GPU work — it runs inside
//! `CustomPaintSource::render()`, where creating a `CommandEncoder` corrupts
//! Vello's in-progress encoder (see `paint_bridge.rs`). Vector layers are
//! therefore rasterised on the CPU into the same premultiplied linear-light
//! buffer the pixel path writes to, so both layer kinds composite together with
//! one set of blending rules.
//!
//! - [`raster`] turns a device-space [`iris_vector::BezPath`] into per-pixel coverage.
//! - [`paint`] evaluates fills and strokes into premultiplied linear RGBA.
//! - [`color`] converts sRGB authoring colours into the linear working space.
//! - [`draw`] applies object transforms, expands strokes, and blends the result.

mod color;
mod draw;
mod edges;
mod paint;
mod raster;

pub(crate) use draw::draw_vector_layer;
