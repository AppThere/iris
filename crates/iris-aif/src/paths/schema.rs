// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! FlatBuffers schema constants for `iris/layers/{id}/paths.bin` (SPEC.md §4.10).
//!
//! Field slot numbers follow FlatBuffers' declaration-order rule: the first
//! field declared in a table is slot 0, the next is slot 1, and so on. A union
//! field occupies **two** consecutive slots — a `ubyte` discriminant followed by
//! the table offset — which is why [`paint::VARIANT_TYPE`] and
//! [`paint::VARIANT`] are adjacent.
//!
//! These constants are the single source of truth shared by [`super::encode`]
//! and [`super::decode`]; neither module may hardcode a slot number.

/// Root table identifier written into bytes 4..8 of the buffer.
pub(crate) const FILE_IDENTIFIER: &str = "AIRF";

/// The only `PathStore.version` this library reads or writes.
pub(crate) const PATH_STORE_VERSION: u32 = 1;

// ── Table field slots ─────────────────────────────────────────────────────────

/// `table PathStore { version, colorSpace, objects }`
pub(crate) mod path_store {
    /// `version: uint = 1`
    pub(crate) const VERSION: u16 = 0;
    /// `colorSpace: string`
    pub(crate) const COLOR_SPACE: u16 = 1;
    /// `objects: [PathObject] (required)`
    pub(crate) const OBJECTS: u16 = 2;
}

/// `table PathObject { id, path, fill, stroke, fillRule, transform, visible, locked, name }`
pub(crate) mod path_object {
    /// `id: [ubyte] (required)` — 16 little-endian UUID bytes.
    pub(crate) const ID: u16 = 0;
    /// `path: PathData (required)`
    pub(crate) const PATH: u16 = 1;
    /// `fill: Paint`
    pub(crate) const FILL: u16 = 2;
    /// `stroke: StrokePaint`
    pub(crate) const STROKE: u16 = 3;
    /// `fillRule: FillRule = NonZero`
    pub(crate) const FILL_RULE: u16 = 4;
    /// `transform: [float]` — 6 elements when present.
    pub(crate) const TRANSFORM: u16 = 5;
    /// `visible: bool = true`
    pub(crate) const VISIBLE: u16 = 6;
    /// `locked: bool = false`
    ///
    // `iris_vector::PathObject` has no `locked` field, so nothing reads or
    // writes this slot yet. The constant is kept because omitting it would make
    // the slot numbering below it look arbitrary — `NAME` is 8 precisely because
    // `locked` occupies 7. Remove the allow when per-object locking lands.
    #[allow(dead_code)]
    pub(crate) const LOCKED: u16 = 7;
    /// `name: string`
    pub(crate) const NAME: u16 = 8;
}

/// `table PathData { verbs, points }`
pub(crate) mod path_data {
    /// `verbs: [byte]`
    pub(crate) const VERBS: u16 = 0;
    /// `points: [float]`
    pub(crate) const POINTS: u16 = 1;
}

/// `table Paint { variant: PaintVariant }` — a union field takes two slots.
pub(crate) mod paint {
    /// Union discriminant (`ubyte`).
    pub(crate) const VARIANT_TYPE: u16 = 0;
    /// Union payload table offset.
    pub(crate) const VARIANT: u16 = 1;
}

/// `table SolidColor { r, g, b, a }`
pub(crate) mod solid_color {
    /// `r: float`
    pub(crate) const R: u16 = 0;
    /// `g: float`
    pub(crate) const G: u16 = 1;
    /// `b: float`
    pub(crate) const B: u16 = 2;
    /// `a: float`
    pub(crate) const A: u16 = 3;
}

/// `table ColorStop { offset, r, g, b, a }`
pub(crate) mod color_stop {
    /// `offset: float`
    pub(crate) const OFFSET: u16 = 0;
    /// `r: float`
    pub(crate) const R: u16 = 1;
    /// `g: float`
    pub(crate) const G: u16 = 2;
    /// `b: float`
    pub(crate) const B: u16 = 3;
    /// `a: float`
    pub(crate) const A: u16 = 4;
}

/// `table LinearGradient { x0, y0, x1, y1, stops, spread }`
pub(crate) mod linear_gradient {
    /// `x0: float`
    pub(crate) const X0: u16 = 0;
    /// `y0: float`
    pub(crate) const Y0: u16 = 1;
    /// `x1: float`
    pub(crate) const X1: u16 = 2;
    /// `y1: float`
    pub(crate) const Y1: u16 = 3;
    /// `stops: [ColorStop]`
    pub(crate) const STOPS: u16 = 4;
    /// `spread: byte`
    pub(crate) const SPREAD: u16 = 5;
}

/// `table RadialGradient { cx, cy, fx, fy, r, stops, spread }`
pub(crate) mod radial_gradient {
    /// `cx: float`
    pub(crate) const CX: u16 = 0;
    /// `cy: float`
    pub(crate) const CY: u16 = 1;
    /// `fx: float`
    pub(crate) const FX: u16 = 2;
    /// `fy: float`
    pub(crate) const FY: u16 = 3;
    /// `r: float`
    pub(crate) const RADIUS: u16 = 4;
    /// `stops: [ColorStop]`
    pub(crate) const STOPS: u16 = 5;
    /// `spread: byte`
    pub(crate) const SPREAD: u16 = 6;
}

/// `table StrokePaint { paint, width, cap, join, miterLimit, dashArray, dashOffset }`
pub(crate) mod stroke_paint {
    /// `paint: Paint`
    pub(crate) const PAINT: u16 = 0;
    /// `width: float`
    pub(crate) const WIDTH: u16 = 1;
    /// `cap: LineCap = Butt`
    pub(crate) const CAP: u16 = 2;
    /// `join: LineJoin = Miter`
    pub(crate) const JOIN: u16 = 3;
    /// `miterLimit: float = 4.0`
    pub(crate) const MITER_LIMIT: u16 = 4;
    /// `dashArray: [float]`
    pub(crate) const DASH_ARRAY: u16 = 5;
    /// `dashOffset: float`
    pub(crate) const DASH_OFFSET: u16 = 6;
}

/// `union PaintVariant { SolidColor, LinearGradient, RadialGradient }` — the
/// implicit `NONE = 0` member is generated by FlatBuffers for every union.
pub(crate) mod paint_variant {
    /// No paint present.
    pub(crate) const NONE: u8 = 0;
    /// Payload is a `SolidColor` table.
    pub(crate) const SOLID: u8 = 1;
    /// Payload is a `LinearGradient` table.
    pub(crate) const LINEAR: u8 = 2;
    /// Payload is a `RadialGradient` table.
    pub(crate) const RADIAL: u8 = 3;
}

/// Default `miterLimit` declared in the schema; omitted from the buffer when
/// the value matches, per FlatBuffers' default-elision rule.
pub(crate) const DEFAULT_MITER_LIMIT: f32 = 4.0;

// ── Path verbs ────────────────────────────────────────────────────────────────

/// `MoveTo` — consumes 2 floats.
pub(crate) const VERB_MOVE_TO: i8 = 0;
/// `LineTo` — consumes 2 floats.
pub(crate) const VERB_LINE_TO: i8 = 1;
/// `QuadTo` — consumes 4 floats.
pub(crate) const VERB_QUAD_TO: i8 = 2;
/// `CubicTo` — consumes 6 floats.
pub(crate) const VERB_CUBIC_TO: i8 = 3;
/// `Close` — consumes 0 floats.
pub(crate) const VERB_CLOSE: i8 = 4;

/// Number of floats a verb consumes from `PathData.points`, or `None` if the
/// verb is not defined in AIF 1.0.
pub(crate) fn verb_arity(verb: i8) -> Option<usize> {
    match verb {
        VERB_MOVE_TO | VERB_LINE_TO => Some(2),
        VERB_QUAD_TO => Some(4),
        VERB_CUBIC_TO => Some(6),
        VERB_CLOSE => Some(0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_arities_match_spec_4_10() {
        assert_eq!(verb_arity(VERB_MOVE_TO), Some(2));
        assert_eq!(verb_arity(VERB_LINE_TO), Some(2));
        assert_eq!(verb_arity(VERB_QUAD_TO), Some(4));
        assert_eq!(verb_arity(VERB_CUBIC_TO), Some(6));
        assert_eq!(verb_arity(VERB_CLOSE), Some(0));
    }

    #[test]
    fn unknown_verbs_have_no_arity() {
        assert_eq!(verb_arity(5), None);
        assert_eq!(verb_arity(-1), None);
    }

    #[test]
    fn union_slots_are_adjacent() {
        assert_eq!(paint::VARIANT, paint::VARIANT_TYPE + 1);
    }
}
