// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Reader-side gradient support: accumulate `<linearGradient>` /
//! `<radialGradient>` definitions and resolve `url(#id)` paint servers into
//! [`iris_vector::Paint`] gradients (SPEC.md §5.4).
//
// COMPAT(inkscape): gradients are commonly split across two elements — geometry
// on one, `<stop>`s on another referenced via `xlink:href`. [`effective`]
// follows that chain so both Illustrator and Inkscape exports resolve.

use std::collections::BTreeMap;

use kurbo::{Point, Rect};
use iris_vector::{ColorStop, LinearGradient, Paint, RadialGradient, SpreadMode};

use crate::attrs::Attrs;
use crate::color::parse_color;

/// Linear vs radial gradient flavour.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GradKind {
    Linear,
    Radial,
}

/// A gradient element captured during parsing: its raw attributes plus the
/// `<stop>`s collected from its children.
pub(crate) struct GradientDef {
    pub kind: GradKind,
    pub attrs: Attrs,
    pub stops: Vec<ColorStop>,
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

/// Parse a `<stop>` element into a [`ColorStop`]. `None` when the stop has no
/// usable colour.
pub(crate) fn parse_stop(a: &Attrs) -> Option<ColorStop> {
    let offset = parse_ratio(a.get("offset").map(String::as_str), 0.0);
    let mut color = parse_color(a.get("stop-color").map(String::as_str).unwrap_or("black"))?;
    let op = a.get("stop-opacity").and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(1.0);
    color.a = (color.a * op).clamp(0.0, 1.0);
    Some(ColorStop { offset: offset.clamp(0.0, 1.0) as f32, color })
}

/// Resolve a `url(#id)` paint reference against the collected `gradients`,
/// using `bbox` (the referencing path's bounds) for objectBoundingBox units and
/// `alpha` as an outer fill/stroke-opacity multiplier.
pub(crate) fn resolve(
    gradients: &BTreeMap<String, GradientDef>,
    reference: &str,
    bbox: Rect,
    alpha: f32,
) -> Option<Paint> {
    let id = ref_id(reference)?;
    let (kind, attrs, mut stops) = effective(gradients, id, 0)?;
    if stops.is_empty() {
        return None;
    }
    if alpha < 1.0 {
        for s in &mut stops {
            s.color.a = (s.color.a * alpha).clamp(0.0, 1.0);
        }
    }
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    if stops.len() == 1 {
        return Some(Paint::Solid(stops[0].color));
    }
    let user = attrs.get("gradientUnits").map(|u| u == "userSpaceOnUse").unwrap_or(false);
    let spread = match attrs.get("spreadMethod").map(String::as_str) {
        Some("reflect") => SpreadMode::Reflect,
        Some("repeat") => SpreadMode::Repeat,
        _ => SpreadMode::Pad,
    };
    Some(match kind {
        GradKind::Linear => Paint::Linear(LinearGradient {
            start: Point::new(coord(&attrs, "x1", 0.0, user, bbox, Axis::X), coord(&attrs, "y1", 0.0, user, bbox, Axis::Y)),
            end: Point::new(coord(&attrs, "x2", 1.0, user, bbox, Axis::X), coord(&attrs, "y2", 0.0, user, bbox, Axis::Y)),
            stops,
            spread,
        }),
        GradKind::Radial => {
            let cx = coord(&attrs, "cx", 0.5, user, bbox, Axis::X);
            let cy = coord(&attrs, "cy", 0.5, user, bbox, Axis::Y);
            Paint::Radial(RadialGradient {
                center: Point::new(cx, cy),
                focus: Point::new(
                    attrs.get("fx").map(|_| coord(&attrs, "fx", 0.5, user, bbox, Axis::X)).unwrap_or(cx),
                    attrs.get("fy").map(|_| coord(&attrs, "fy", 0.5, user, bbox, Axis::Y)).unwrap_or(cy),
                ),
                radius: radius(&attrs, user, bbox),
                stops,
                spread,
            })
        }
    })
}

/// Build the effective gradient by following `xlink:href` inheritance: stops are
/// inherited when absent, and geometry attributes fill in where unset.
fn effective(
    gradients: &BTreeMap<String, GradientDef>,
    id: &str,
    depth: u8,
) -> Option<(GradKind, Attrs, Vec<ColorStop>)> {
    let def = gradients.get(id)?;
    let mut attrs = def.attrs.clone();
    let mut stops = def.stops.clone();
    if depth < 8 {
        if let Some(href) = def.attrs.get("xlink:href").or_else(|| def.attrs.get("href")) {
            if let Some(parent_id) = href.strip_prefix('#') {
                if let Some((_, p_attrs, p_stops)) = effective(gradients, parent_id, depth + 1) {
                    if stops.is_empty() {
                        stops = p_stops;
                    }
                    for (k, v) in p_attrs {
                        attrs.entry(k).or_insert(v);
                    }
                }
            }
        }
    }
    Some((def.kind, attrs, stops))
}

/// Extract the id from `url(#id)`, tolerating quotes and whitespace.
fn ref_id(reference: &str) -> Option<&str> {
    let inner = reference.trim().strip_prefix("url(")?.strip_suffix(')')?;
    Some(inner.trim().trim_matches(['"', '\'']).trim_start_matches('#'))
}

/// Parse a ratio that may be a bare number or a percentage (`"50%"` → 0.5).
fn parse_ratio(s: Option<&str>, default: f64) -> f64 {
    match s {
        None => default,
        Some(v) => {
            let t = v.trim();
            match t.strip_suffix('%') {
                Some(p) => p.trim().parse::<f64>().map(|n| n / 100.0).unwrap_or(default),
                None => t.parse::<f64>().unwrap_or(default),
            }
        }
    }
}

/// Materialise a coordinate attribute into absolute object space.
fn coord(a: &Attrs, key: &str, default_frac: f64, user: bool, bbox: Rect, axis: Axis) -> f64 {
    let (origin, span) = match axis {
        Axis::X => (bbox.x0, bbox.width()),
        Axis::Y => (bbox.y0, bbox.height()),
    };
    match a.get(key) {
        None => origin + default_frac * span,
        Some(v) => {
            let t = v.trim();
            if let Some(p) = t.strip_suffix('%') {
                origin + p.trim().parse::<f64>().unwrap_or(default_frac * 100.0) / 100.0 * span
            } else {
                let n = t.parse::<f64>().unwrap_or(default_frac);
                if user { n } else { origin + n * span }
            }
        }
    }
}

/// Materialise a radius. objectBoundingBox radii scale by the bbox diagonal
/// (`√(w²+h²)/√2`); our model is circular so this is exact only for square
/// bounds (a documented approximation).
fn radius(a: &Attrs, user: bool, bbox: Rect) -> f64 {
    let diag = (bbox.width().powi(2) + bbox.height().powi(2)).sqrt() / std::f64::consts::SQRT_2;
    match a.get("r") {
        None => 0.5 * diag,
        Some(v) => {
            let t = v.trim();
            if let Some(p) = t.strip_suffix('%') {
                p.trim().parse::<f64>().unwrap_or(50.0) / 100.0 * diag
            } else {
                let n = t.parse::<f64>().unwrap_or(0.5);
                if user { n } else { n * diag }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_vector::Color;

    fn attrs(pairs: &[(&str, &str)]) -> Attrs {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn grad(kind: GradKind, a: &[(&str, &str)], stops: Vec<ColorStop>) -> GradientDef {
        GradientDef { kind, attrs: attrs(a), stops }
    }

    fn two_stops() -> Vec<ColorStop> {
        vec![
            ColorStop { offset: 0.0, color: Color::new(1.0, 0.0, 0.0, 1.0) },
            ColorStop { offset: 1.0, color: Color::new(0.0, 0.0, 1.0, 1.0) },
        ]
    }

    #[test]
    fn stop_offset_percentage_and_opacity() {
        let s = parse_stop(&attrs(&[("offset", "50%"), ("stop-color", "#ff0000"), ("stop-opacity", "0.5")])).unwrap();
        assert!((s.offset - 0.5).abs() < 1e-6);
        assert!((s.color.a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn user_space_linear_uses_absolute_coords() {
        let mut g = BTreeMap::new();
        g.insert(
            "grad".into(),
            grad(GradKind::Linear, &[("gradientUnits", "userSpaceOnUse"), ("x1", "10"), ("y1", "0"), ("x2", "90"), ("y2", "0")], two_stops()),
        );
        let bbox = Rect::new(0.0, 0.0, 100.0, 100.0);
        let Some(Paint::Linear(lg)) = resolve(&g, "url(#grad)", bbox, 1.0) else {
            panic!("expected linear gradient");
        };
        assert_eq!(lg.start, Point::new(10.0, 0.0));
        assert_eq!(lg.end, Point::new(90.0, 0.0));
        assert_eq!(lg.stops.len(), 2);
    }

    #[test]
    fn object_bbox_linear_maps_into_bounds() {
        let mut g = BTreeMap::new();
        // Default objectBoundingBox: x1=0%,x2=100% across the bbox.
        g.insert("grad".into(), grad(GradKind::Linear, &[], two_stops()));
        let bbox = Rect::new(20.0, 30.0, 120.0, 130.0); // 100×100
        let Some(Paint::Linear(lg)) = resolve(&g, "url(#grad)", bbox, 1.0) else {
            panic!("expected linear gradient");
        };
        assert_eq!(lg.start, Point::new(20.0, 30.0));
        assert_eq!(lg.end, Point::new(120.0, 30.0));
    }

    #[test]
    fn single_stop_collapses_to_solid() {
        let mut g = BTreeMap::new();
        let stops = vec![ColorStop { offset: 0.0, color: Color::new(0.2, 0.4, 0.6, 1.0) }];
        g.insert("grad".into(), grad(GradKind::Linear, &[], stops));
        assert!(matches!(resolve(&g, "url(#grad)", Rect::new(0.0, 0.0, 1.0, 1.0), 1.0), Some(Paint::Solid(_))));
    }

    #[test]
    fn href_inherits_stops() {
        let mut g = BTreeMap::new();
        g.insert("base".into(), grad(GradKind::Linear, &[], two_stops()));
        g.insert(
            "geo".into(),
            grad(GradKind::Linear, &[("xlink:href", "#base"), ("gradientUnits", "userSpaceOnUse"), ("x2", "50")], vec![]),
        );
        let Some(Paint::Linear(lg)) = resolve(&g, "url(#geo)", Rect::new(0.0, 0.0, 100.0, 100.0), 1.0) else {
            panic!("expected linear gradient with inherited stops");
        };
        assert_eq!(lg.stops.len(), 2);
        assert_eq!(lg.end.x, 50.0);
    }

    #[test]
    fn unknown_reference_is_none() {
        let g: BTreeMap<String, GradientDef> = BTreeMap::new();
        assert!(resolve(&g, "url(#missing)", Rect::new(0.0, 0.0, 1.0, 1.0), 1.0).is_none());
    }
}
