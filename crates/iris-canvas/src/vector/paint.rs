// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Paint evaluation for vector rasterisation.
//!
//! Every value produced here is converted to premultiplied linear light by
//! [`super::color`] before it reaches the accumulation buffer.
//!
//! Gradient stops interpolate in sRGB, matching SVG 1.1 §13.2, and are converted
//! to linear afterwards — interpolating in linear space would shift the midpoint
//! of every gradient relative to what the authoring tool showed.

use iris_vector::{Affine, Color, ColorStop, Paint, Point, SpreadMode};

use super::color::{invertible, premultiplied_linear};

/// A paint resolved into device space, ready for per-pixel evaluation.
pub(super) enum DevicePaint {
    /// A constant premultiplied linear RGBA value.
    Solid([f32; 4]),
    /// A gradient evaluated per pixel in object space.
    Gradient {
        /// Maps a device-space point back to object space.
        device_to_object: Affine,
        kind: GradientKind,
        stops: Vec<ColorStop>,
        spread: SpreadMode,
        /// Layer opacity folded in, applied after stop interpolation.
        opacity: f32,
    },
}

/// Gradient geometry, in object space.
pub(super) enum GradientKind {
    /// Projection onto the segment `start → end`.
    Linear { start: Point, end: Point },
    /// Ratio along the ray from `focus` through the sample point to the circle.
    Radial { center: Point, focus: Point, radius: f64 },
}

impl DevicePaint {
    /// Resolve an `iris-vector` paint against the object → device transform.
    ///
    /// Returns `None` when the transform is singular (a degenerate object
    /// transform or zero zoom), since gradients cannot be inverted through it.
    pub(super) fn new(paint: &Paint, object_to_device: Affine, opacity: f32) -> Option<Self> {
        match paint {
            Paint::Solid(c) => Some(Self::Solid(premultiplied_linear(*c, opacity))),
            Paint::Linear(g) => Some(Self::Gradient {
                device_to_object: invertible(object_to_device)?,
                kind: GradientKind::Linear { start: g.start, end: g.end },
                stops: g.stops.clone(),
                spread: g.spread,
                opacity,
            }),
            Paint::Radial(g) => Some(Self::Gradient {
                device_to_object: invertible(object_to_device)?,
                kind: GradientKind::Radial {
                    center: g.center,
                    focus: g.focus,
                    radius: g.radius,
                },
                stops: g.stops.clone(),
                spread: g.spread,
                opacity,
            }),
        }
    }

    /// Evaluate the paint at a device-space pixel centre, returning
    /// premultiplied linear RGBA.
    pub(super) fn sample(&self, dx: f64, dy: f64) -> [f32; 4] {
        match self {
            Self::Solid(rgba) => *rgba,
            Self::Gradient { device_to_object, kind, stops, spread, opacity } => {
                let p = *device_to_object * Point::new(dx, dy);
                let t = match kind {
                    GradientKind::Linear { start, end } => linear_t(p, *start, *end),
                    GradientKind::Radial { center, focus, radius } => {
                        radial_t(p, *center, *focus, *radius)
                    }
                };
                premultiplied_linear(sample_stops(stops, apply_spread(t, *spread)), *opacity)
            }
        }
    }
}

/// Return `t` such that `p` projects onto `start → end` at that fraction.
fn linear_t(p: Point, start: Point, end: Point) -> f64 {
    let d = end - start;
    let len_sq = d.hypot2();
    if len_sq <= f64::EPSILON {
        // Degenerate gradient: SVG renders it as the last stop's colour.
        return 1.0;
    }
    (p - start).dot(d) / len_sq
}

/// Two-point conical gradient: the fraction along the ray `focus → p` at which
/// the ray meets the circle `(center, radius)`.
///
/// Solves `|fc + s·pd| = radius` for `s > 0` and returns `1/s`, so a point on
/// the circle yields `1.0` and the focus itself yields `0.0`. With
/// `focus == center` this reduces to `|p − center| / radius`.
fn radial_t(p: Point, center: Point, focus: Point, radius: f64) -> f64 {
    if radius <= f64::EPSILON {
        return 1.0;
    }
    let pd = p - focus;
    let fc = focus - center;
    let a = pd.hypot2();
    if a <= f64::EPSILON {
        return 0.0; // exactly at the focus
    }
    let b = 2.0 * fc.dot(pd);
    let c = fc.hypot2() - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        // Focus outside the circle and the ray misses it entirely.
        return 1.0;
    }
    let s = (-b + disc.sqrt()) / (2.0 * a);
    if s <= f64::EPSILON {
        1.0
    } else {
        1.0 / s
    }
}

/// Map `t` into `0.0..=1.0` according to the gradient's spread mode.
fn apply_spread(t: f64, spread: SpreadMode) -> f64 {
    match spread {
        SpreadMode::Pad => t.clamp(0.0, 1.0),
        SpreadMode::Repeat => t.rem_euclid(1.0),
        SpreadMode::Reflect => {
            let m = t.rem_euclid(2.0);
            if m > 1.0 {
                2.0 - m
            } else {
                m
            }
        }
    }
}

/// Interpolate the stop list at `t`, in sRGB.
fn sample_stops(stops: &[ColorStop], t: f64) -> Color {
    let Some(first) = stops.first() else {
        // No stops: SVG treats this as "none". Fully transparent.
        return Color::new(0.0, 0.0, 0.0, 0.0);
    };
    let t = t as f32;
    if t <= first.offset {
        return first.color;
    }
    // `stops` is documented as ordered by offset; scan for the bracketing pair.
    for pair in stops.windows(2) {
        let (lo, hi) = (&pair[0], &pair[1]);
        if t <= hi.offset {
            let span = hi.offset - lo.offset;
            let f = if span <= f32::EPSILON { 0.0 } else { (t - lo.offset) / span };
            return Color::new(
                lo.color.r + (hi.color.r - lo.color.r) * f,
                lo.color.g + (hi.color.g - lo.color.g) * f,
                lo.color.b + (hi.color.b - lo.color.b) * f,
                lo.color.a + (hi.color.a - lo.color.a) * f,
            );
        }
    }
    // Past the last stop.
    stops.last().map_or(first.color, |s| s.color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stops() -> Vec<ColorStop> {
        vec![
            ColorStop { offset: 0.0, color: Color::new(0.0, 0.0, 0.0, 1.0) },
            ColorStop { offset: 1.0, color: Color::new(1.0, 1.0, 1.0, 1.0) },
        ]
    }

    #[test]
    fn solid_paint_is_premultiplied() {
        let p = DevicePaint::new(&Paint::Solid(Color::new(1.0, 1.0, 1.0, 0.5)), Affine::IDENTITY, 1.0)
            .expect("identity is invertible");
        let [r, _, _, a] = p.sample(0.0, 0.0);
        assert!((a - 0.5).abs() < 1e-6);
        assert!((r - 0.5).abs() < 1e-6, "white at 50% alpha premultiplies to 0.5");
    }

    #[test]
    fn layer_opacity_scales_alpha() {
        let p = DevicePaint::new(&Paint::Solid(Color::new(0.0, 0.0, 0.0, 1.0)), Affine::IDENTITY, 0.25)
            .expect("invertible");
        assert!((p.sample(0.0, 0.0)[3] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn linear_t_projects_onto_the_axis() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert!((linear_t(Point::new(0.0, 0.0), a, b) - 0.0).abs() < 1e-9);
        assert!((linear_t(Point::new(5.0, 3.0), a, b) - 0.5).abs() < 1e-9);
        assert!((linear_t(Point::new(10.0, 0.0), a, b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn radial_t_is_distance_ratio_when_focus_is_centred() {
        let c = Point::new(0.0, 0.0);
        assert!((radial_t(Point::new(5.0, 0.0), c, c, 10.0) - 0.5).abs() < 1e-9);
        assert!((radial_t(Point::new(10.0, 0.0), c, c, 10.0) - 1.0).abs() < 1e-9);
        assert!(radial_t(c, c, c, 10.0).abs() < 1e-9);
    }

    #[test]
    fn radial_t_with_offset_focus_still_hits_one_on_the_circle() {
        let c = Point::new(0.0, 0.0);
        let f = Point::new(3.0, 0.0);
        // A point on the circle must map to exactly 1.0 regardless of focus.
        assert!((radial_t(Point::new(10.0, 0.0), c, f, 10.0) - 1.0).abs() < 1e-9);
        assert!((radial_t(Point::new(0.0, 10.0), c, f, 10.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn spread_modes_map_out_of_range_t() {
        assert_eq!(apply_spread(1.5, SpreadMode::Pad), 1.0);
        assert_eq!(apply_spread(-0.5, SpreadMode::Pad), 0.0);
        assert!((apply_spread(1.25, SpreadMode::Repeat) - 0.25).abs() < 1e-9);
        assert!((apply_spread(1.25, SpreadMode::Reflect) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn stop_sampling_interpolates_and_clamps() {
        let s = stops();
        assert_eq!(sample_stops(&s, -1.0).r, 0.0);
        assert_eq!(sample_stops(&s, 2.0).r, 1.0);
        assert!((sample_stops(&s, 0.5).r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn empty_stop_list_is_transparent() {
        assert_eq!(sample_stops(&[], 0.5).a, 0.0);
    }

    #[test]
    fn singular_transform_rejects_gradients() {
        let g = iris_vector::LinearGradient {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
            stops: stops(),
            spread: SpreadMode::Pad,
        };
        let squashed = Affine::scale_non_uniform(0.0, 1.0);
        assert!(DevicePaint::new(&Paint::Linear(g), squashed, 1.0).is_none());
    }
}
