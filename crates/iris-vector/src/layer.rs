// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`VectorLayer`] — the payload of a vector layer: an ordered list of
//! [`PathObject`]s plus the colour space their paints are authored in.

use crate::object::PathObject;

/// The contents of a vector layer.
#[derive(Debug, Clone)]
pub struct VectorLayer {
    /// Path objects, ordered bottom-to-top within the layer (index 0 is drawn
    /// first / lowest).
    pub objects: Vec<PathObject>,
    /// Colour space the object paints are expressed in (SPEC.md §4.9; e.g.
    /// `"srgb"`). Vector paints are authored in sRGB.
    pub color_space: String,
}

impl VectorLayer {
    /// An empty vector layer in the given colour space.
    pub fn new(color_space: impl Into<String>) -> Self {
        Self { objects: Vec::new(), color_space: color_space.into() }
    }
}

impl Default for VectorLayer {
    fn default() -> Self {
        Self::new("srgb")
    }
}
