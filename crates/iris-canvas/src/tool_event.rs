// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

/// A normalised tool event from the canvas, in document space.
///
/// All `doc_pos` coordinates are in document pixels — origin at the document
/// top-left, axes identical to screen axes at zero rotation. Callers should
/// use [`crate::viewport::CanvasViewport::screen_to_doc`] to convert from
/// raw pointer positions before constructing these variants.
#[derive(Debug, Clone)]
pub enum ToolEvent {
    /// Pointer pressed on the canvas (mouse button down or stylus touch).
    Down {
        doc_pos: kurbo::Vec2,
        /// Normalised pressure 0.0–1.0. Mouse always reports 1.0.
        pressure: f32,
        /// Stylus tilt in degrees around the X axis. Mouse always reports 0.0.
        tilt_x: f32,
        /// Stylus tilt in degrees around the Y axis. Mouse always reports 0.0.
        tilt_y: f32,
        button: PointerButton,
    },
    /// Pointer moved while at least one button was held.
    Move {
        doc_pos: kurbo::Vec2,
        pressure: f32,
        tilt_x: f32,
        tilt_y: f32,
    },
    /// Pointer button released.
    Up {
        doc_pos: kurbo::Vec2,
        button: PointerButton,
    },
    /// Scroll-wheel or two-finger pan delta, in screen pixels.
    ///
    /// Positive Y = scroll down. The canvas should translate `pan` in the
    /// opposite direction so content follows the finger.
    Scroll { delta_x: f32, delta_y: f32 },
    /// Pinch-to-zoom gesture. `scale_factor > 1.0` means zoom in.
    ///
    /// `anchor_screen` is the screen-space fixed point (midpoint of the two
    /// touch contacts). The viewport should zoom around this point.
    Pinch { scale_factor: f32, anchor_screen: kurbo::Vec2 },
}

/// Which pointer button generated a [`ToolEvent::Down`] or [`ToolEvent::Up`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}
