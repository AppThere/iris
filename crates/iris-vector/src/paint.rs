// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Paint types for vector fills and stroke colours: solid colours and linear /
//! radial gradients.

use kurbo::Point;

/// An sRGB colour with straight (non-premultiplied) alpha, components in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component.
    pub r: f32,
    /// Green component.
    pub g: f32,
    /// Blue component.
    pub b: f32,
    /// Alpha component.
    pub a: f32,
}

impl Color {
    /// Construct a colour from sRGB components in `0.0..=1.0`.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Construct an opaque colour from 8-bit sRGB components.
    pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }

    /// Opaque black.
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
}

/// How a gradient is extended beyond its `0.0..=1.0` stop range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMode {
    /// Clamp to the nearest stop.
    Pad,
    /// Mirror the gradient.
    Reflect,
    /// Tile the gradient.
    Repeat,
}

/// A single colour stop in a gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    /// Position along the gradient, `0.0..=1.0`.
    pub offset: f32,
    /// Stop colour.
    pub color: Color,
}

/// A linear gradient from `start` to `end`.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    /// Gradient start point in object space.
    pub start: Point,
    /// Gradient end point in object space.
    pub end: Point,
    /// Colour stops, ordered by `offset`.
    pub stops: Vec<ColorStop>,
    /// Extension behaviour beyond the stop range.
    pub spread: SpreadMode,
}

/// A radial gradient centred at `center` with the given `radius`.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    /// Outer circle centre in object space.
    pub center: Point,
    /// Focal point in object space (often equal to `center`).
    pub focus: Point,
    /// Outer circle radius.
    pub radius: f64,
    /// Colour stops, ordered by `offset`.
    pub stops: Vec<ColorStop>,
    /// Extension behaviour beyond the stop range.
    pub spread: SpreadMode,
}

/// A fill or stroke paint.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// A single flat colour.
    Solid(Color),
    /// A linear gradient.
    Linear(LinearGradient),
    /// A radial gradient.
    Radial(RadialGradient),
    // TODO(iris): SPEC.md §3.3 — mesh gradients and pattern paints (Phase 3+).
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rgb8_maps_to_unit_range() {
        let c = Color::from_rgb8(255, 0, 128);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert!((c.b - 0.5019608).abs() < 1e-6);
        assert_eq!(c.a, 1.0);
    }
}
