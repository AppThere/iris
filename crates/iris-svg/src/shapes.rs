// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Convert SVG basic-shape elements into kurbo [`BezPath`] geometry.

use std::fmt::Write;

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
    let (rx, ry) = corner_radii(a, w, h);
    if rx <= 0.0 || ry <= 0.0 {
        return Some(Rect::new(x, y, x + w, y + h).to_path(TOL));
    }
    rounded_rect_path(x, y, w, h, rx, ry)
}

/// Resolve `rx`/`ry` per the SVG rect rules: a missing radius inherits the
/// other, negatives are ignored, and each is clamped to half the side length.
fn corner_radii(a: &Attrs, w: f64, h: f64) -> (f64, f64) {
    let rx_attr = f64_of(a, "rx").filter(|v| *v >= 0.0);
    let ry_attr = f64_of(a, "ry").filter(|v| *v >= 0.0);
    let (rx, ry) = match (rx_attr, ry_attr) {
        (None, None) => (0.0, 0.0),
        (Some(rx), None) => (rx, rx),
        (None, Some(ry)) => (ry, ry),
        (Some(rx), Some(ry)) => (rx, ry),
    };
    (rx.min(w / 2.0), ry.min(h / 2.0))
}

/// Build a rounded-rectangle path with elliptical (`rx` ≠ `ry`) corners. The
/// `A` arc commands are parsed by kurbo, which approximates them with Béziers.
fn rounded_rect_path(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> Option<BezPath> {
    let (x2, y2) = (x + w, y + h);
    let mut d = String::new();
    let _ = write!(d, "M{} {} ", x + rx, y);
    let _ = write!(d, "H{} ", x2 - rx);
    let _ = write!(d, "A{rx} {ry} 0 0 1 {} {} ", x2, y + ry);
    let _ = write!(d, "V{} ", y2 - ry);
    let _ = write!(d, "A{rx} {ry} 0 0 1 {} {} ", x2 - rx, y2);
    let _ = write!(d, "H{} ", x + rx);
    let _ = write!(d, "A{rx} {ry} 0 0 1 {} {} ", x, y2 - ry);
    let _ = write!(d, "V{} ", y + ry);
    let _ = write!(d, "A{rx} {ry} 0 0 1 {} {} Z", x + rx, y);
    BezPath::from_svg(&d).ok()
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
    fn sharp_rect_has_no_curves() {
        let p = shape_to_path(b"rect", &attrs(&[("width", "10"), ("height", "5")])).unwrap();
        assert!(!p.elements().iter().any(|e| matches!(e, kurbo::PathEl::CurveTo(..))));
    }

    #[test]
    fn rounded_rect_has_curves_and_stays_in_bounds() {
        let p = shape_to_path(
            b"rect",
            &attrs(&[("width", "100"), ("height", "60"), ("rx", "10"), ("ry", "8")]),
        )
        .unwrap();
        assert!(p.elements().iter().any(|e| matches!(e, kurbo::PathEl::CurveTo(..))), "rounded corners produce curves");
        let bb = p.bounding_box();
        assert!(bb.x0 >= -0.5 && bb.y0 >= -0.5 && bb.x1 <= 100.5 && bb.y1 <= 60.5, "stays within the rect: {bb:?}");
    }

    #[test]
    fn single_radius_is_inherited_and_clamped() {
        // ry inherits rx; both clamp to half the side (rx 80 → 50, ry → 30).
        let p = shape_to_path(
            b"rect",
            &attrs(&[("width", "100"), ("height", "60"), ("rx", "80")]),
        )
        .unwrap();
        assert!(p.elements().iter().any(|e| matches!(e, kurbo::PathEl::CurveTo(..))));
        let bb = p.bounding_box();
        assert!(bb.width() <= 100.5 && bb.height() <= 60.5, "clamped radii keep bounds: {bb:?}");
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
