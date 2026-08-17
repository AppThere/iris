// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Flattening a Bezier path into the edge list the scanline rasteriser consumes.

use iris_vector::{BezPath, PathEl, Point};

/// Flattening tolerance in device pixels. Curves are subdivided until they
/// deviate by less than this from the true Bézier.
const FLATTEN_TOLERANCE: f64 = 0.1;

/// A non-horizontal line segment, normalised so `y0 < y1`.
pub(super) struct Edge {
    pub(super) x0: f64,
    pub(super) y0: f64,
    pub(super) x1: f64,
    pub(super) y1: f64,
    /// `+1` if the original segment pointed down, `-1` if up. Drives non-zero winding.
    pub(super) dir: i32,
}

impl Edge {
    /// X coordinate where this edge crosses the horizontal line `y`.
    pub(super) fn x_at(&self, y: f64) -> f64 {
        let dy = self.y1 - self.y0;
        // `dy > 0` by construction (horizontal segments are never stored).
        self.x0 + (y - self.y0) * (self.x1 - self.x0) / dy
    }
}

/// Flatten `path` into normalised edges, implicitly closing every subpath.
pub(super) fn build_edges(path: &BezPath) -> Vec<Edge> {
    let mut edges = Vec::new();
    let mut start: Option<Point> = None;
    let mut current: Option<Point> = None;

    let push = |a: Point, b: Point, edges: &mut Vec<Edge>| {
        if a.y == b.y {
            return; // horizontal edges never produce crossings
        }
        let (p0, p1, dir) = if a.y < b.y { (a, b, 1) } else { (b, a, -1) };
        edges.push(Edge { x0: p0.x, y0: p0.y, x1: p1.x, y1: p1.y, dir });
    };

    kurbo::flatten(path.elements().iter().copied(), FLATTEN_TOLERANCE, |el| match el {
        PathEl::MoveTo(p) => {
            // An unclosed previous subpath is closed implicitly for filling.
            if let (Some(s), Some(c)) = (start, current) {
                push(c, s, &mut edges);
            }
            start = Some(p);
            current = Some(p);
        }
        PathEl::LineTo(p) => {
            if let Some(c) = current {
                push(c, p, &mut edges);
            }
            current = Some(p);
        }
        PathEl::ClosePath => {
            if let (Some(s), Some(c)) = (start, current) {
                push(c, s, &mut edges);
            }
            current = start;
        }
        // `kurbo::flatten` emits only MoveTo / LineTo / ClosePath.
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });

    if let (Some(s), Some(c)) = (start, current) {
        push(c, s, &mut edges);
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(4.0, 0.0));
        p.line_to(Point::new(4.0, 4.0));
        p.line_to(Point::new(0.0, 4.0));
        p.close_path();
        p
    }

    #[test]
    fn horizontal_edges_are_dropped() {
        // A square has 4 sides but only the 2 vertical ones can cross a scanline.
        assert_eq!(build_edges(&square()).len(), 2);
    }

    #[test]
    fn edges_are_normalised_downward_with_direction_preserved() {
        let edges = build_edges(&square());
        assert!(edges.iter().all(|e| e.y0 < e.y1), "every edge must run downward");
        let sum: i32 = edges.iter().map(|e| e.dir).sum();
        assert_eq!(sum, 0, "a closed loop has balanced winding directions");
    }

    #[test]
    fn unclosed_subpaths_are_closed_implicitly() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(4.0, 0.0));
        p.line_to(Point::new(4.0, 4.0));
        assert_eq!(build_edges(&p).len(), 2, "the closing edge must be synthesised");
    }

    #[test]
    fn empty_path_has_no_edges() {
        assert!(build_edges(&BezPath::new()).is_empty());
    }

    #[test]
    fn x_at_interpolates_along_the_edge() {
        let e = Edge { x0: 0.0, y0: 0.0, x1: 10.0, y1: 10.0, dir: 1 };
        assert!((e.x_at(5.0) - 5.0).abs() < 1e-9);
    }
}
