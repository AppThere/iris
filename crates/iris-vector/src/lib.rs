// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Vector document model: path objects, paints, strokes, and vector layers.
//!
//! These types are the in-memory representation of vector content. A
//! [`VectorLayer`] is carried by `iris_pixel::LayerContent::Vector`, so the
//! unified layer tree holds both raster and vector layers (ADR 001, SPEC §3.3).
//! Format adapters (`iris-svg`, `iris-ai`) read into and write out of this model.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod layer;
mod object;
mod paint;
mod stroke;

pub use layer::VectorLayer;
pub use object::{ObjectId, PathObject};
pub use paint::{Color, ColorStop, LinearGradient, Paint, RadialGradient, SpreadMode};
pub use stroke::{FillRule, LineCap, LineJoin, StrokePaint};

// Re-export the kurbo geometry types adapters need so they can depend on
// iris-vector alone for the vector model.
pub use kurbo::{Affine, BezPath, Point};
