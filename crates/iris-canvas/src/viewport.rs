// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`CanvasViewport`] — infinite-canvas transform: pan, zoom, rotation.
//!
//! Pan convention (Q1 decision): `pan` is the document-space coordinate
//! visible at the **screen center**. Rotation is always around the screen
//! center. The spec's original "top-left" comment was rejected because that
//! convention is undefined when rotation is non-zero.

/// Minimum allowed zoom factor (10%).
pub const MIN_ZOOM: f32 = 0.1;
/// Maximum allowed zoom factor (6400%).
pub const MAX_ZOOM: f32 = 64.0;

/// Infinite-canvas viewport transform.
///
/// Maps between document space (pixels, origin at top-left of the document)
/// and screen space (pixels, origin at top-left of the screen).
///
/// The transform is a similarity: translate → rotate → scale, anchored at
/// the screen center.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasViewport {
    /// Document-space coordinate visible at the screen center.
    /// Changing `pan` translates the canvas without affecting zoom or rotation.
    pub pan: kurbo::Vec2,
    /// Zoom factor. 1.0 = 100%, 0.1 = 10% (MIN_ZOOM), 64.0 = 6400% (MAX_ZOOM).
    pub zoom: f32,
    /// Canvas rotation in radians, counter-clockwise, applied around the screen center.
    /// 0.0 = upright (default). Non-destructive: document pixels are never modified.
    pub rotation: f32,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasViewport {
    /// Create a default viewport (zoom = 1.0, pan = origin, no rotation).
    pub fn new() -> Self {
        Self {
            pan: kurbo::Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
        }
    }

    /// Convert a screen-space point to document space.
    ///
    /// Transform sequence (screen-center anchor, Q1):
    /// 1. Translate by `−screen_center` → relative to screen center
    /// 2. Rotate by `−self.rotation` (undo canvas rotation)
    /// 3. Scale by `1/zoom`
    /// 4. Translate by `+self.pan` → document-space result
    pub fn screen_to_doc(&self, screen: kurbo::Vec2, sw: u32, sh: u32) -> kurbo::Vec2 {
        let center = kurbo::Vec2::new(sw as f64 * 0.5, sh as f64 * 0.5);
        let rel = screen - center;
        let (sin, cos) = ((-self.rotation) as f64).sin_cos();
        let rotated = kurbo::Vec2::new(
            rel.x * cos - rel.y * sin,
            rel.x * sin + rel.y * cos,
        );
        self.pan + rotated / self.zoom as f64
    }

    /// Convert a document-space point to screen space. Inverse of [`screen_to_doc`].
    ///
    /// Transform sequence:
    /// 1. Translate by `−self.pan` → relative to document anchor
    /// 2. Scale by `zoom`
    /// 3. Rotate by `+self.rotation`
    /// 4. Translate by `+screen_center`
    pub fn doc_to_screen(&self, doc: kurbo::Vec2, sw: u32, sh: u32) -> kurbo::Vec2 {
        let center = kurbo::Vec2::new(sw as f64 * 0.5, sh as f64 * 0.5);
        let rel = (doc - self.pan) * self.zoom as f64;
        let (sin, cos) = (self.rotation as f64).sin_cos();
        let rotated = kurbo::Vec2::new(
            rel.x * cos - rel.y * sin,
            rel.x * sin + rel.y * cos,
        );
        center + rotated
    }

    /// Axis-aligned bounding box in document space covering all visible screen pixels.
    ///
    /// At rotation = 0 this is a tight rect. At rotation ≠ 0 it is the AABB of
    /// the four rotated viewport corners — always a superset of visible pixels.
    pub fn visible_doc_rect(&self, sw: u32, sh: u32) -> kurbo::Rect {
        let corners = [
            self.screen_to_doc(kurbo::Vec2::new(0.0, 0.0), sw, sh),
            self.screen_to_doc(kurbo::Vec2::new(sw as f64, 0.0), sw, sh),
            self.screen_to_doc(kurbo::Vec2::new(0.0, sh as f64), sw, sh),
            self.screen_to_doc(kurbo::Vec2::new(sw as f64, sh as f64), sw, sh),
        ];
        let x0 = corners.iter().map(|v| v.x).fold(f64::INFINITY, f64::min);
        let y0 = corners.iter().map(|v| v.y).fold(f64::INFINITY, f64::min);
        let x1 = corners.iter().map(|v| v.x).fold(f64::NEG_INFINITY, f64::max);
        let y1 = corners.iter().map(|v| v.y).fold(f64::NEG_INFINITY, f64::max);
        kurbo::Rect::new(x0, y0, x1, y1)
    }

    /// Zoom towards/away from `anchor_screen`, keeping that screen point fixed in doc space.
    ///
    /// `new_zoom` is clamped to `[MIN_ZOOM, MAX_ZOOM]`. After this call, the doc-space
    /// coordinate under `anchor_screen` is identical to before.
    pub fn zoom_to(&mut self, new_zoom: f32, anchor_screen: kurbo::Vec2, sw: u32, sh: u32) {
        let new_zoom = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let anchor_doc = self.screen_to_doc(anchor_screen, sw, sh);
        self.zoom = new_zoom;
        // Recompute pan so anchor_doc maps back to anchor_screen after the zoom change.
        let center = kurbo::Vec2::new(sw as f64 * 0.5, sh as f64 * 0.5);
        let rel = anchor_screen - center;
        let (sin, cos) = ((-self.rotation) as f64).sin_cos();
        let rotated = kurbo::Vec2::new(
            rel.x * cos - rel.y * sin,
            rel.x * sin + rel.y * cos,
        );
        self.pan = anchor_doc - rotated / new_zoom as f64;
    }

    /// Clamp zoom to the valid range without changing pan or rotation.
    pub fn clamp_zoom(&mut self) {
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    const SW: u32 = 256;
    const SH: u32 = 256;
    const TOL: f64 = 1e-9;

    fn approx_eq(a: kurbo::Vec2, b: kurbo::Vec2) -> bool {
        (a.x - b.x).abs() < TOL && (a.y - b.y).abs() < TOL
    }

    #[test]
    fn round_trip_zoom1_no_rotation() {
        let vp = CanvasViewport::new();
        for pt in [
            kurbo::Vec2::new(0.0, 0.0),
            kurbo::Vec2::new(128.0, 128.0),
            kurbo::Vec2::new(256.0, 256.0),
        ] {
            let doc = vp.screen_to_doc(pt, SW, SH);
            let back = vp.doc_to_screen(doc, SW, SH);
            assert!(approx_eq(pt, back), "round-trip failed for {pt:?}");
        }
    }

    #[test]
    fn round_trip_zoom2() {
        let vp = CanvasViewport { zoom: 2.0, ..CanvasViewport::new() };
        let pt = kurbo::Vec2::new(64.0, 200.0);
        let back = vp.doc_to_screen(vp.screen_to_doc(pt, SW, SH), SW, SH);
        assert!(approx_eq(pt, back));
    }

    #[test]
    fn round_trip_rotation_quarter_pi() {
        let vp = CanvasViewport { rotation: FRAC_PI_4, ..CanvasViewport::new() };
        let pt = kurbo::Vec2::new(10.0, 240.0);
        let back = vp.doc_to_screen(vp.screen_to_doc(pt, SW, SH), SW, SH);
        assert!(approx_eq(pt, back));
    }

    #[test]
    fn visible_doc_rect_covers_all_corners() {
        let vp = CanvasViewport { rotation: FRAC_PI_4, zoom: 0.5, ..CanvasViewport::new() };
        let rect = vp.visible_doc_rect(SW, SH);
        for screen in [
            kurbo::Vec2::new(0.0, 0.0),
            kurbo::Vec2::new(SW as f64, 0.0),
            kurbo::Vec2::new(0.0, SH as f64),
            kurbo::Vec2::new(SW as f64, SH as f64),
        ] {
            let doc = vp.screen_to_doc(screen, SW, SH);
            assert!(rect.x0 - TOL <= doc.x && doc.x <= rect.x1 + TOL);
            assert!(rect.y0 - TOL <= doc.y && doc.y <= rect.y1 + TOL);
        }
    }

    #[test]
    fn rotation_widens_visible_rect() {
        let vp_flat = CanvasViewport::new();
        let vp_rot = CanvasViewport { rotation: FRAC_PI_4, ..CanvasViewport::new() };
        let flat = vp_flat.visible_doc_rect(SW, SH);
        let rot = vp_rot.visible_doc_rect(SW, SH);
        let flat_area = (flat.x1 - flat.x0) * (flat.y1 - flat.y0);
        let rot_area = (rot.x1 - rot.x0) * (rot.y1 - rot.y0);
        assert!(rot_area > flat_area, "rotated rect should have larger AABB");
    }

    #[test]
    fn zoom_clamp() {
        let mut vp = CanvasViewport::new();
        vp.zoom_to(0.0, kurbo::Vec2::new(128.0, 128.0), SW, SH);
        assert!((vp.zoom - MIN_ZOOM).abs() < f32::EPSILON);
        vp.zoom_to(1000.0, kurbo::Vec2::new(128.0, 128.0), SW, SH);
        assert!((vp.zoom - MAX_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn zoom_anchor_stable() {
        let mut vp = CanvasViewport { zoom: 1.0, ..CanvasViewport::new() };
        let anchor_screen = kurbo::Vec2::new(80.0, 60.0);
        let anchor_doc_before = vp.screen_to_doc(anchor_screen, SW, SH);
        vp.zoom_to(3.0, anchor_screen, SW, SH);
        let anchor_doc_after = vp.screen_to_doc(anchor_screen, SW, SH);
        assert!(
            approx_eq(anchor_doc_before, anchor_doc_after),
            "anchor moved: before={anchor_doc_before:?} after={anchor_doc_after:?}"
        );
    }
}
