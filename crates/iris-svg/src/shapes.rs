// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Convert SVG basic-shape elements into kurbo [`BezPath`] geometry.

use kurbo::{BezPath, Circle, Ellipse, Point, Rect, Shape};

use crate::attrs::{f64_of, str_of, parse_len, Attrs};

/// Flattening tolerance when converting analytic shapes to Béziers.
const TOL: f64 = 0.1;

/// Build a path for a shape element, or `None` if the tag is not a basic shape
/// or required attributes are missing.
pub(crate) fn shape_to_path(tag: &[u8], a: &Attrs) -> Option<BezPath> {
    match tag {
        b"path" => str_of(a, "d").and_then(|d| BezPath::from_svg(d).ok()),
        b"rect" => rect(a),
        b"circle" => circle(a),
        b"ellipse" => ellipse(a),
        b"line" => line(a),
        b"polyline" => poly(a, false),
        b"polygon" => poly(a, true),
        _ => None,
    }
}

fn rect(a: &Attrs) -> Option<BezPath> {
    let x = f64_of(a, "x").unwrap_or(0.0);
    let y = f64_of(a, "y").unwrap_or(0.0);
    let w = f64_of(a, "width")?;
    let h = f64_of(a, "height")?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    // TODO(iris): SPEC.md §5.4 — honour rx/ry for rounded rects.
    Some(Rect::new(x, y, x + w, y + h).to_path(TOL))
}

fn circle(a: &Attrs) -> Option<BezPath> {
    let cx = f64_of(a, "cx").unwrap_or(0.0);
    let cy = f64_of(a, "cy").unwrap_or(0.0);
    let r = f64_of(a, "r")?;
    (r > 0.0).then(|| Circle::new((cx, cy), r).to_path(TOL))
}

fn ellipse(a: &Attrs) -> Option<BezPath> {
    let cx = f64_of(a, "cx").unwrap_or(0.0);
    let cy = f64_of(a, "cy").unwrap_or(0.0);
    let rx = f64_of(a, "rx")?;
    let ry = f64_of(a, "ry")?;
    (rx > 0.0 && ry > 0.0).then(|| Ellipse::new((cx, cy), (rx, ry), 0.0).to_path(TOL))
}

fn line(a: &Attrs) -> Option<BezPath> {
    let x1 = f64_of(a, "x1").unwrap_or(0.0);
    let y1 = f64_of(a, "y1").unwrap_or(0.0);
    let x2 = f64_of(a, "x2").unwrap_or(0.0);
    let y2 = f64_of(a, "y2").unwrap_or(0.0);
    let mut p = BezPath::new();
    p.move_to(Point::new(x1, y1));
    p.line_to(Point::new(x2, y2));
    Some(p)
}

fn poly(a: &Attrs, close: bool) -> Option<BezPath> {
    let pts = parse_points(str_of(a, "points")?);
    if pts.len() < 2 {
        return None;
    }
    let mut p = BezPath::new();
    p.move_to(pts[0]);
    for pt in &pts[1..] {
        p.line_to(*pt);
    }
    if close {
        p.close_path();
    }
    Some(p)
}

fn parse_points(s: &str) -> Vec<Point> {
    let nums: Vec<f64> = s
        .split([',', ' ', '\t', '\n', '\r'])
        .filter(|t| !t.is_empty())
        .filter_map(parse_len)
        .collect();
    nums.chunks_exact(2).map(|c| Point::new(c[0], c[1])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> Attrs {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn rect_produces_closed_path() {
        let p = shape_to_path(b"rect", &attrs(&[("width", "10"), ("height", "5")])).unwrap();
        assert!(p.elements().len() >= 4);
    }

    #[test]
    fn polygon_parses_points() {
        let p = shape_to_path(b"polygon", &attrs(&[("points", "0,0 10,0 10,10")])).unwrap();
        assert!(!p.elements().is_empty());
    }

    #[test]
    fn path_uses_d_attribute() {
        let p = shape_to_path(b"path", &attrs(&[("d", "M0 0 L10 10")])).unwrap();
        assert!(!p.elements().is_empty());
    }
}
