// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Colour-space conversion between `iris-vector`'s authoring space and the
//! compositor's working space.
//!
//! `iris-vector` authors colours in **sRGB with straight alpha**; the compositor
//! accumulates in **premultiplied linear light** (ADR 003). Everything crossing
//! that boundary converts here.

use iris_vector::{Affine, Color};

/// Convert an sRGB straight-alpha colour to premultiplied linear RGBA,
/// folding in `opacity`.
pub(super) fn premultiplied_linear(c: Color, opacity: f32) -> [f32; 4] {
    let a = (c.a * opacity).clamp(0.0, 1.0);
    [
        srgb_to_linear(c.r) * a,
        srgb_to_linear(c.g) * a,
        srgb_to_linear(c.b) * a,
        a,
    ]
}

/// Decode an sRGB-gamma [0,1] value to linear light.
///
/// Inverse of [`crate::compositor::encode::to_srgb_u8`].
pub(crate) fn srgb_to_linear(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    if s <= 0.040_449_936 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Return `a.inverse()` unless `a` is singular.
///
/// A degenerate object transform or zero zoom cannot be inverted, and gradients
/// need the inverse to map device pixels back into object space.
pub(super) fn invertible(a: Affine) -> Option<Affine> {
    let det = a.determinant();
    if det.abs() <= f64::EPSILON || !det.is_finite() {
        return None;
    }
    Some(a.inverse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::encode::to_srgb_u8;

    #[test]
    fn srgb_to_linear_matches_known_points() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        // sRGB 0.5 is roughly linear 0.2140.
        assert!((srgb_to_linear(0.5) - 0.214_041).abs() < 1e-4);
    }

    #[test]
    fn srgb_round_trips_through_the_encoder() {
        for step in 0..=20 {
            let s = step as f32 / 20.0;
            let encoded = to_srgb_u8(srgb_to_linear(s));
            let expected = (s * 255.0 + 0.5) as u8;
            assert!(
                encoded.abs_diff(expected) <= 1,
                "sRGB {s} decoded then re-encoded to {encoded}, expected ~{expected}"
            );
        }
    }

    #[test]
    fn premultiplication_scales_colour_by_alpha() {
        let [r, _, _, a] = premultiplied_linear(Color::new(1.0, 1.0, 1.0, 0.5), 1.0);
        assert!((a - 0.5).abs() < 1e-6);
        assert!((r - 0.5).abs() < 1e-6, "white at 50% alpha premultiplies to 0.5");
    }

    #[test]
    fn opacity_folds_into_alpha() {
        let [_, _, _, a] = premultiplied_linear(Color::new(0.0, 0.0, 0.0, 1.0), 0.25);
        assert!((a - 0.25).abs() < 1e-6);
    }

    #[test]
    fn singular_transforms_are_rejected() {
        assert!(invertible(Affine::scale_non_uniform(0.0, 1.0)).is_none());
        assert!(invertible(Affine::IDENTITY).is_some());
    }
}
