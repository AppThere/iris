// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Writer-side gradient support: collect [`iris_vector::Paint`] gradients while
//! serialising and emit a `<defs>` block of `<linearGradient>` /
//! `<radialGradient>` elements (SPEC.md §5.4).
//!
//! Gradients are written with `gradientUnits="userSpaceOnUse"` and absolute
//! coordinates, matching the object-space gradient model the reader produces, so
//! a write → read round-trip is lossless.

use iris_vector::{ColorStop, Paint, SpreadMode};

use crate::color::to_hex;

/// Accumulates gradient definitions and hands out unique ids for references.
#[derive(Default)]
pub(crate) struct GradientRegistry {
    defs: String,
    count: usize,
}

impl GradientRegistry {
    /// Register a paint. Returns `Some(id)` for a gradient — the caller emits
    /// `fill="url(#id)"` / `stroke="url(#id)"` — or `None` for a solid colour.
    pub(crate) fn register(&mut self, paint: &Paint) -> Option<String> {
        match paint {
            Paint::Solid(_) => None,
            Paint::Linear(g) => {
                let id = self.next_id();
                self.defs.push_str(&format!(
                    "    <linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" \
                     x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{}>\n",
                    g.start.x, g.start.y, g.end.x, g.end.y, spread_attr(g.spread)
                ));
                self.push_stops(&g.stops);
                self.defs.push_str("    </linearGradient>\n");
                Some(id)
            }
            Paint::Radial(g) => {
                let id = self.next_id();
                self.defs.push_str(&format!(
                    "    <radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" \
                     cx=\"{}\" cy=\"{}\" r=\"{}\" fx=\"{}\" fy=\"{}\"{}>\n",
                    g.center.x, g.center.y, g.radius, g.focus.x, g.focus.y, spread_attr(g.spread)
                ));
                self.push_stops(&g.stops);
                self.defs.push_str("    </radialGradient>\n");
                Some(id)
            }
        }
    }

    /// The `<defs>` block to emit, or `None` if no gradients were registered.
    pub(crate) fn defs_block(&self) -> Option<String> {
        if self.defs.is_empty() {
            None
        } else {
            Some(format!("  <defs>\n{}  </defs>\n", self.defs))
        }
    }

    fn next_id(&mut self) -> String {
        let id = format!("iris-grad-{}", self.count);
        self.count += 1;
        id
    }

    fn push_stops(&mut self, stops: &[ColorStop]) {
        for s in stops {
            let mut line = format!(
                "      <stop offset=\"{}\" stop-color=\"{}\"",
                s.offset,
                to_hex(s.color)
            );
            if s.color.a < 1.0 {
                line.push_str(&format!(" stop-opacity=\"{}\"", s.color.a));
            }
            line.push_str("/>\n");
            self.defs.push_str(&line);
        }
    }
}

fn spread_attr(spread: SpreadMode) -> &'static str {
    match spread {
        SpreadMode::Pad => "",
        SpreadMode::Reflect => " spreadMethod=\"reflect\"",
        SpreadMode::Repeat => " spreadMethod=\"repeat\"",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_vector::{Color, LinearGradient};
    use kurbo::Point;

    fn linear() -> Paint {
        Paint::Linear(LinearGradient {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            stops: vec![
                ColorStop { offset: 0.0, color: Color::new(1.0, 0.0, 0.0, 1.0) },
                ColorStop { offset: 1.0, color: Color::new(0.0, 0.0, 1.0, 0.5) },
            ],
            spread: SpreadMode::Reflect,
        })
    }

    #[test]
    fn solid_is_not_registered() {
        let mut r = GradientRegistry::default();
        assert!(r.register(&Paint::Solid(Color::BLACK)).is_none());
        assert!(r.defs_block().is_none());
    }

    #[test]
    fn linear_emits_defs_with_unique_ids() {
        let mut r = GradientRegistry::default();
        let id0 = r.register(&linear()).unwrap();
        let id1 = r.register(&linear()).unwrap();
        assert_ne!(id0, id1);
        let defs = r.defs_block().unwrap();
        assert!(defs.contains("<linearGradient"));
        assert!(defs.contains("spreadMethod=\"reflect\""));
        assert!(defs.contains("stop-opacity=\"0.5\""));
        assert!(defs.contains(&format!("id=\"{id0}\"")));
    }
}
