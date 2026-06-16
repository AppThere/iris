// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Stroke and fill-rule types.

use crate::paint::Paint;

/// Determines how a path's interior is computed for filling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Non-zero winding rule (SVG `nonzero`).
    #[default]
    NonZero,
    /// Even-odd rule (SVG `evenodd`).
    EvenOdd,
}

/// Stroke line-cap style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Flat cap flush with the endpoint.
    #[default]
    Butt,
    /// Rounded cap.
    Round,
    /// Square cap extending past the endpoint.
    Square,
}

/// Stroke line-join style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// Sharp mitred corner.
    #[default]
    Miter,
    /// Rounded corner.
    Round,
    /// Bevelled corner.
    Bevel,
}

/// A stroke: its paint and geometry parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokePaint {
    /// The paint applied along the stroke.
    pub paint: Paint,
    /// Stroke width in object-space units.
    pub width: f64,
    /// Line-cap style.
    pub cap: LineCap,
    /// Line-join style.
    pub join: LineJoin,
    /// Miter limit for [`LineJoin::Miter`].
    pub miter_limit: f64,
    /// Dash lengths (empty = solid).
    pub dash_array: Vec<f64>,
    /// Dash phase offset.
    pub dash_offset: f64,
}

impl StrokePaint {
    /// A solid stroke of the given paint and width with default join/cap.
    pub fn new(paint: Paint, width: f64) -> Self {
        Self {
            paint,
            width,
            cap: LineCap::default(),
            join: LineJoin::default(),
            miter_limit: 4.0,
            dash_array: Vec::new(),
            dash_offset: 0.0,
        }
    }
}
