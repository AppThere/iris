// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Anti-aliased scanline rasteriser for device-space Bézier paths.
//!
//! The path is flattened to line segments, then each output row is sampled at
//! [`SUBSAMPLES`] sub-scanlines. Crossings along a sub-scanline are sorted and
//! walked to produce interior spans under the requested fill rule; span ends
//! contribute fractional horizontal coverage. The result is exact horizontal
//! anti-aliasing with `SUBSAMPLES`-step vertical anti-aliasing.
//!
//! Edges are kept in a y-sorted list with an active set carried across rows, so
//! cost scales with edges *crossing the current row* rather than with the whole
//! path. Edge construction lives in [`super::edges`].

use iris_vector::BezPath;

use super::edges::build_edges;

/// Vertical sub-samples per output row.
const SUBSAMPLES: usize = 4;

/// Rasterise `path` (already in device space) and report per-pixel coverage.
///
/// `plot` receives `(x, y, coverage)` for every pixel with non-zero coverage,
/// where coverage is in `0.0..=1.0`. Pixels outside `width × height` are clipped.
pub(super) fn fill_path(
    path: &BezPath,
    even_odd: bool,
    width: u32,
    height: u32,
    mut plot: impl FnMut(usize, usize, f32),
) {
    let mut edges = build_edges(path);
    if edges.is_empty() || width == 0 || height == 0 {
        return;
    }

    // Clip the sweep to the intersection of the path's y-extent and the buffer.
    let min_y = edges.iter().fold(f64::INFINITY, |m, e| m.min(e.y0));
    let max_y = edges.iter().fold(f64::NEG_INFINITY, |m, e| m.max(e.y1));
    let row_start = (min_y.floor().max(0.0)) as usize;
    let row_end = (max_y.ceil().max(0.0) as usize).min(height as usize);
    if row_start >= row_end {
        return;
    }

    edges.sort_by(|a, b| a.y0.total_cmp(&b.y0));

    let mut coverage = vec![0.0_f32; width as usize];
    let mut active: Vec<usize> = Vec::new();
    let mut next = 0usize;
    let mut crossings: Vec<(f64, i32)> = Vec::new();
    let inv_sub = 1.0 / SUBSAMPLES as f32;

    for row in row_start..row_end {
        let row_top = row as f64;
        let row_bottom = row_top + 1.0;

        // Admit edges that start before this row ends, retire those already passed.
        while next < edges.len() && edges[next].y0 < row_bottom {
            active.push(next);
            next += 1;
        }
        active.retain(|&i| edges[i].y1 > row_top);
        if active.is_empty() {
            continue;
        }

        coverage.iter_mut().for_each(|c| *c = 0.0);
        for sub in 0..SUBSAMPLES {
            let sample_y = row_top + (sub as f64 + 0.5) / SUBSAMPLES as f64;
            crossings.clear();
            for &i in &active {
                let e = &edges[i];
                if e.y0 <= sample_y && sample_y < e.y1 {
                    crossings.push((e.x_at(sample_y), e.dir));
                }
            }
            if crossings.len() < 2 {
                continue;
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            accumulate_spans(&crossings, even_odd, inv_sub, width, &mut coverage);
        }

        for (x, &c) in coverage.iter().enumerate() {
            if c > 0.0 {
                plot(x, row, c.min(1.0));
            }
        }
    }
}

/// Walk sorted crossings, emitting interior spans into `coverage`.
fn accumulate_spans(
    crossings: &[(f64, i32)],
    even_odd: bool,
    weight: f32,
    width: u32,
    coverage: &mut [f32],
) {
    let mut winding = 0i32;
    let mut span_start = 0.0_f64;
    for (i, &(x, dir)) in crossings.iter().enumerate() {
        let was_inside = is_inside(winding, even_odd);
        winding += if even_odd { 1 } else { dir };
        let now_inside = is_inside(winding, even_odd);
        if !was_inside && now_inside {
            span_start = x;
        } else if was_inside && !now_inside {
            add_span(span_start, x, weight, width, coverage);
        }
        // A trailing unmatched crossing cannot open a span that ever closes.
        let _ = i;
    }
}

/// Whether the accumulated winding number counts as "inside".
fn is_inside(winding: i32, even_odd: bool) -> bool {
    if even_odd {
        winding % 2 != 0
    } else {
        winding != 0
    }
}

/// Add `weight` coverage across `[x0, x1)`, with fractional ends.
fn add_span(x0: f64, x1: f64, weight: f32, width: u32, coverage: &mut [f32]) {
    let w = width as f64;
    let x0 = x0.max(0.0);
    let x1 = x1.min(w);
    if x1 <= x0 {
        return;
    }
    let first = x0.floor() as usize;
    let last = (x1.ceil() as usize).min(width as usize);
    if first >= coverage.len() {
        return;
    }
    if last - first == 1 {
        // Span lies within a single pixel column.
        coverage[first] += weight * (x1 - x0) as f32;
        return;
    }
    for (px, slot) in coverage.iter_mut().enumerate().take(last).skip(first) {
        let left = (px as f64).max(x0);
        let right = ((px + 1) as f64).min(x1);
        if right > left {
            *slot += weight * (right - left) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_vector::Point;

    /// Rasterise into a dense coverage buffer for assertions.
    fn raster(path: &BezPath, even_odd: bool, w: u32, h: u32) -> Vec<f32> {
        let mut buf = vec![0.0_f32; (w * h) as usize];
        fill_path(path, even_odd, w, h, |x, y, c| buf[y * w as usize + x] = c);
        buf
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x0, y0));
        p.line_to(Point::new(x1, y0));
        p.line_to(Point::new(x1, y1));
        p.line_to(Point::new(x0, y1));
        p.close_path();
        p
    }

    #[test]
    fn axis_aligned_rect_is_fully_covered_inside() {
        let buf = raster(&rect(2.0, 2.0, 6.0, 6.0), false, 8, 8);
        for y in 2..6 {
            for x in 2..6 {
                assert!(
                    (buf[y * 8 + x] - 1.0).abs() < 1e-3,
                    "pixel ({x},{y}) should be fully covered, got {}",
                    buf[y * 8 + x]
                );
            }
        }
    }

    #[test]
    fn outside_the_rect_is_untouched() {
        let buf = raster(&rect(2.0, 2.0, 6.0, 6.0), false, 8, 8);
        for (i, &c) in buf.iter().enumerate() {
            let (x, y) = (i % 8, i / 8);
            let inside = (2..6).contains(&x) && (2..6).contains(&y);
            if !inside {
                assert_eq!(c, 0.0, "pixel ({x},{y}) outside the rect must stay clear");
            }
        }
    }

    #[test]
    fn half_pixel_edge_gives_partial_coverage() {
        // A rect ending at x = 4.5 covers half of column 4.
        let buf = raster(&rect(2.0, 2.0, 4.5, 6.0), false, 8, 8);
        let c = buf[3 * 8 + 4];
        assert!((c - 0.5).abs() < 1e-3, "expected ~0.5 coverage, got {c}");
    }

    #[test]
    fn unclosed_subpath_is_filled_as_if_closed() {
        let mut p = BezPath::new();
        p.move_to(Point::new(2.0, 2.0));
        p.line_to(Point::new(6.0, 2.0));
        p.line_to(Point::new(6.0, 6.0));
        p.line_to(Point::new(2.0, 6.0));
        // deliberately no close_path()
        let buf = raster(&p, false, 8, 8);
        assert!((buf[3 * 8 + 3] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn nonzero_and_even_odd_differ_on_nested_rects() {
        // Two nested rects wound the same way: non-zero fills the hole, even-odd
        // punches it out.
        let mut p = rect(0.0, 0.0, 8.0, 8.0);
        p.extend(rect(2.0, 2.0, 6.0, 6.0).elements().iter().copied());

        let nz = raster(&p, false, 8, 8);
        assert!((nz[4 * 8 + 4] - 1.0).abs() < 1e-3, "non-zero must fill the centre");

        let eo = raster(&p, true, 8, 8);
        assert!(eo[4 * 8 + 4] < 1e-3, "even-odd must leave the centre empty");
    }

    #[test]
    fn geometry_outside_the_buffer_is_clipped_not_panicking() {
        let buf = raster(&rect(-50.0, -50.0, 100.0, 100.0), false, 8, 8);
        assert!(buf.iter().all(|c| (c - 1.0).abs() < 1e-3), "buffer fully covered");
    }

    #[test]
    fn empty_and_degenerate_paths_are_no_ops() {
        assert!(raster(&BezPath::new(), false, 8, 8).iter().all(|&c| c == 0.0));
        // Zero-height rect: only horizontal edges, which never cross a scanline.
        assert!(raster(&rect(1.0, 3.0, 5.0, 3.0), false, 8, 8).iter().all(|&c| c == 0.0));
    }

    #[test]
    fn curves_are_flattened_and_filled() {
        let mut p = BezPath::new();
        p.move_to(Point::new(4.0, 1.0));
        p.curve_to(Point::new(7.0, 1.0), Point::new(7.0, 7.0), Point::new(4.0, 7.0));
        p.curve_to(Point::new(1.0, 7.0), Point::new(1.0, 1.0), Point::new(4.0, 1.0));
        p.close_path();
        let buf = raster(&p, false, 8, 8);
        assert!(buf[4 * 8 + 4] > 0.9, "centre of the closed curve must be filled");
    }
}
