// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Resolve SVG presentation attributes into iris-vector paints and stroke.

use iris_vector::{Color, FillRule, LineCap, LineJoin, Paint, StrokePaint};

use crate::attrs::{f64_of, str_of, Attrs};
use crate::color::parse_color;

/// Presentation properties that inherit from a parent element.
const INHERITED: &[&str] = &[
    "fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin",
    "stroke-miterlimit", "stroke-dasharray", "stroke-dashoffset", "fill-rule",
    "opacity", "fill-opacity", "stroke-opacity", "color",
];

/// Produce the effective styling map for an element: the inherited properties
/// overlaid with this element's own inheritable properties.
pub(crate) fn merge(parent: &Attrs, own: &Attrs) -> Attrs {
    let mut out = parent.clone();
    for &key in INHERITED {
        if let Some(v) = own.get(key) {
            out.insert(key.to_string(), v.clone());
        }
    }
    out
}

fn opacity(s: &Attrs, key: &str) -> f32 {
    let elem = f64_of(s, "opacity").unwrap_or(1.0) as f32;
    let specific = f64_of(s, key).unwrap_or(1.0) as f32;
    (elem * specific).clamp(0.0, 1.0)
}

/// Resolve the fill paint. SVG's initial fill is opaque black; `fill="none"`
/// and unsupported paint servers (e.g. `url(#…)`) yield no fill.
pub(crate) fn fill(s: &Attrs) -> Option<Paint> {
    let alpha = opacity(s, "fill-opacity");
    match str_of(s, "fill") {
        None => Some(Paint::Solid(with_alpha(Color::BLACK, alpha))),
        Some(v) if v.starts_with("url(") => {
            // TODO(iris): SPEC.md §5.4 — resolve gradient/pattern paint servers.
            tracing::warn!(paint = v, "SVG paint server not yet supported; leaving unfilled");
            None
        }
        Some(v) => parse_color(v).map(|c| Paint::Solid(with_alpha(c, alpha))),
    }
}

/// Resolve the stroke. SVG's initial stroke is none.
pub(crate) fn stroke(s: &Attrs) -> Option<StrokePaint> {
    let color = parse_color(str_of(s, "stroke")?)?;
    let alpha = opacity(s, "stroke-opacity");
    Some(StrokePaint {
        paint: Paint::Solid(with_alpha(color, alpha)),
        width: f64_of(s, "stroke-width").unwrap_or(1.0),
        cap: match str_of(s, "stroke-linecap") {
            Some("round") => LineCap::Round,
            Some("square") => LineCap::Square,
            _ => LineCap::Butt,
        },
        join: match str_of(s, "stroke-linejoin") {
            Some("round") => LineJoin::Round,
            Some("bevel") => LineJoin::Bevel,
            _ => LineJoin::Miter,
        },
        miter_limit: f64_of(s, "stroke-miterlimit").unwrap_or(4.0),
        dash_array: dash_array(s),
        dash_offset: f64_of(s, "stroke-dashoffset").unwrap_or(0.0),
    })
}

/// Resolve the fill rule (`nonzero` default, or `evenodd`).
pub(crate) fn fill_rule(s: &Attrs) -> FillRule {
    match str_of(s, "fill-rule") {
        Some("evenodd") => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    }
}

fn dash_array(s: &Attrs) -> Vec<f64> {
    match str_of(s, "stroke-dasharray") {
        Some(v) if v != "none" => v
            .split([',', ' '])
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse().ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn with_alpha(mut c: Color, a: f32) -> Color {
    c.a = a;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> Attrs {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn default_fill_is_black() {
        assert_eq!(fill(&attrs(&[])), Some(Paint::Solid(Color::BLACK)));
    }

    #[test]
    fn fill_none_yields_no_fill_and_stroke_parses() {
        let s = attrs(&[("fill", "none"), ("stroke", "#ff0000"), ("stroke-width", "2")]);
        assert_eq!(fill(&s), None);
        let st = stroke(&s).unwrap();
        assert_eq!(st.width, 2.0);
    }

    #[test]
    fn inheritance_overlays_child() {
        let parent = attrs(&[("fill", "red"), ("opacity", "0.5")]);
        let child = attrs(&[("fill", "blue")]);
        let merged = merge(&parent, &child);
        assert_eq!(merged.get("fill").map(String::as_str), Some("blue"));
        assert_eq!(merged.get("opacity").map(String::as_str), Some("0.5"));
    }
}
