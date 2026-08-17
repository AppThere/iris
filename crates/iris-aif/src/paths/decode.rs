// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Decoder for `iris/layers/{id}/paths.bin` (SPEC.md §4.10).
//!
//! Every structural failure maps to a variant of the §4.15 taxonomy; the
//! decoder never panics on malformed input (SPEC.md §4.1 rule 2).

use uuid::Uuid;

use crate::error::AifError;
use iris_vector::{
    Affine, BezPath, Color, ColorStop, FillRule, LineCap, LineJoin, LinearGradient, Paint,
    PathObject, Point, RadialGradient, SpreadMode, StrokePaint, VectorLayer,
};

use super::schema::{
    color_stop, linear_gradient, paint, paint_variant, path_data, path_object, path_store,
    radial_gradient, solid_color, stroke_paint, verb_arity, DEFAULT_MITER_LIMIT, FILE_IDENTIFIER,
    PATH_STORE_VERSION,
};
use super::wire::{root, Table};

/// Decode `paths.bin` bytes into a [`VectorLayer`].
///
/// `meta_color_space` is the layer's `colorSpace` from `meta.xml` when one is
/// present; a mismatch against `PathStore.colorSpace` is
/// [`AifError::ColorSpaceMismatch`] per §4.10.
///
// AUDIT: SPEC.md §4.6 defines `colorSpace` only on `<iris:PixelData>`, which is
// pixel-only, so a conformant vector layer's meta.xml carries no colour space to
// compare against — yet §4.10 and §4.15 both require the cross-check. Rather
// than invent a `<iris:VectorData>` element (§4.1 rule 1 forbids inferring
// schema), `PathStore.colorSpace` is treated as authoritative and the check runs
// only when a caller supplies one. Resolving this needs a SPEC change.
pub(crate) fn read_path_store(
    bytes: &[u8],
    layer_id: Uuid,
    meta_color_space: Option<&str>,
) -> Result<VectorLayer, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };
    let store = root(bytes, FILE_IDENTIFIER).map_err(|_| malformed())?;

    let version = store.u32_field(path_store::VERSION, PATH_STORE_VERSION).map_err(|_| malformed())?;
    if version > PATH_STORE_VERSION {
        // AUDIT: SPEC.md §4.16 rule 6 says a reader should skip a newer path
        // store and render the layer empty with a warning, but §4.10 and the
        // §4.15 fatal-error table both make this a hard error. Following the
        // normative reader rule in §4.10; §4.16 rule 6 needs reconciling.
        return Err(AifError::UnknownPathStoreVersion { layer_id, version });
    }

    let color_space = store
        .string_field(path_store::COLOR_SPACE)
        .map_err(|_| malformed())?
        .unwrap_or("srgb")
        .to_string();
    if let Some(meta) = meta_color_space {
        if meta != color_space {
            return Err(AifError::ColorSpaceMismatch { layer_id });
        }
    }

    let tables = store
        .table_vector(path_store::OBJECTS)
        .map_err(|_| malformed())?
        .ok_or_else(malformed)?;

    let mut objects = Vec::with_capacity(tables.len());
    let mut seen: Vec<Uuid> = Vec::with_capacity(tables.len());
    for t in tables {
        let obj = decode_object(&t, layer_id)?;
        // §4.10: object ids must be unique within a PathStore.
        if seen.contains(&obj.id) {
            return Err(malformed());
        }
        seen.push(obj.id);
        objects.push(obj);
    }

    Ok(VectorLayer { objects, color_space })
}

// ── Objects ───────────────────────────────────────────────────────────────────

fn decode_object(t: &Table<'_>, layer_id: Uuid) -> Result<PathObject, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };

    let id_bytes = t.bytes_vector(path_object::ID).map_err(|_| malformed())?.ok_or_else(malformed)?;
    let id_array: [u8; 16] = id_bytes.try_into().map_err(|_| malformed())?;
    let id = Uuid::from_bytes_le(id_array);

    let path_table =
        t.table_field(path_object::PATH).map_err(|_| malformed())?.ok_or_else(malformed)?;
    let path = decode_path(&path_table, layer_id)?;

    let fill = match t.table_field(path_object::FILL).map_err(|_| malformed())? {
        Some(p) => decode_paint(&p, layer_id)?,
        None => None,
    };
    let stroke = match t.table_field(path_object::STROKE).map_err(|_| malformed())? {
        Some(s) => decode_stroke(&s, layer_id)?,
        None => None,
    };

    let transform = match t.f32_vector(path_object::TRANSFORM).map_err(|_| malformed())? {
        Some(v) if v.len() == 6 => {
            Affine::new([v[0] as f64, v[1] as f64, v[2] as f64, v[3] as f64, v[4] as f64, v[5] as f64])
        }
        Some(v) => return Err(AifError::MalformedTransform { layer_id, got: v.len() }),
        None => Affine::IDENTITY,
    };

    Ok(PathObject {
        id,
        path,
        fill,
        stroke,
        fill_rule: fill_rule_from(t.i8_field(path_object::FILL_RULE, 0).map_err(|_| malformed())?),
        transform,
        name: t
            .string_field(path_object::NAME)
            .map_err(|_| malformed())?
            .unwrap_or_default()
            .to_string(),
        visible: t.bool_field(path_object::VISIBLE, true).map_err(|_| malformed())?,
    })
}

fn decode_path(t: &Table<'_>, layer_id: Uuid) -> Result<BezPath, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };
    let verbs = t.bytes_vector(path_data::VERBS).map_err(|_| malformed())?.unwrap_or(&[]);
    let points = t.f32_vector(path_data::POINTS).map_err(|_| malformed())?.unwrap_or_default();

    // §4.10: points length must equal the summed arity of the verb sequence.
    let needed: usize = verbs
        .iter()
        .map(|v| verb_arity(*v as i8).ok_or_else(malformed))
        .sum::<Result<usize, AifError>>()?;
    if needed != points.len() {
        return Err(malformed());
    }

    let mut path = BezPath::new();
    let mut i = 0usize;
    let pt = |i: &mut usize| {
        let p = Point::new(points[*i] as f64, points[*i + 1] as f64);
        *i += 2;
        p
    };
    for v in verbs {
        match *v as i8 {
            super::schema::VERB_MOVE_TO => path.move_to(pt(&mut i)),
            super::schema::VERB_LINE_TO => path.line_to(pt(&mut i)),
            super::schema::VERB_QUAD_TO => {
                let (a, b) = (pt(&mut i), pt(&mut i));
                path.quad_to(a, b);
            }
            super::schema::VERB_CUBIC_TO => {
                let (a, b, c) = (pt(&mut i), pt(&mut i), pt(&mut i));
                path.curve_to(a, b, c);
            }
            _ => path.close_path(),
        }
    }
    Ok(path)
}

// ── Paints ────────────────────────────────────────────────────────────────────

fn decode_paint(t: &Table<'_>, layer_id: Uuid) -> Result<Option<Paint>, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };
    let tag = t.u8_field(paint::VARIANT_TYPE, paint_variant::NONE).map_err(|_| malformed())?;
    if tag == paint_variant::NONE {
        return Ok(None);
    }
    let v = t.table_field(paint::VARIANT).map_err(|_| malformed())?.ok_or_else(malformed)?;

    let paint = match tag {
        paint_variant::SOLID => Paint::Solid(Color::new(
            v.f32_field(solid_color::R, 0.0).map_err(|_| malformed())?,
            v.f32_field(solid_color::G, 0.0).map_err(|_| malformed())?,
            v.f32_field(solid_color::B, 0.0).map_err(|_| malformed())?,
            v.f32_field(solid_color::A, 0.0).map_err(|_| malformed())?,
        )),
        paint_variant::LINEAR => Paint::Linear(LinearGradient {
            start: xy(&v, linear_gradient::X0, linear_gradient::Y0, layer_id)?,
            end: xy(&v, linear_gradient::X1, linear_gradient::Y1, layer_id)?,
            stops: decode_stops(&v, linear_gradient::STOPS, layer_id)?,
            spread: spread_from(v.i8_field(linear_gradient::SPREAD, 0).map_err(|_| malformed())?),
        }),
        paint_variant::RADIAL => Paint::Radial(RadialGradient {
            center: xy(&v, radial_gradient::CX, radial_gradient::CY, layer_id)?,
            focus: xy(&v, radial_gradient::FX, radial_gradient::FY, layer_id)?,
            radius: v.f32_field(radial_gradient::RADIUS, 0.0).map_err(|_| malformed())? as f64,
            stops: decode_stops(&v, radial_gradient::STOPS, layer_id)?,
            spread: spread_from(v.i8_field(radial_gradient::SPREAD, 0).map_err(|_| malformed())?),
        }),
        other => {
            // §4.16 rule 3: an unrecognised optional value degrades to absent.
            tracing::warn!(
                layer_id = %layer_id,
                variant = other,
                "unknown PaintVariant in paths.bin; treating as unpainted"
            );
            return Ok(None);
        }
    };
    Ok(Some(paint))
}

fn decode_stroke(t: &Table<'_>, layer_id: Uuid) -> Result<Option<StrokePaint>, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };
    let paint_table =
        t.table_field(stroke_paint::PAINT).map_err(|_| malformed())?.ok_or_else(malformed)?;
    let Some(paint) = decode_paint(&paint_table, layer_id)? else { return Ok(None) };

    Ok(Some(StrokePaint {
        paint,
        width: t.f32_field(stroke_paint::WIDTH, 0.0).map_err(|_| malformed())? as f64,
        cap: cap_from(t.i8_field(stroke_paint::CAP, 0).map_err(|_| malformed())?),
        join: join_from(t.i8_field(stroke_paint::JOIN, 0).map_err(|_| malformed())?),
        miter_limit: t
            .f32_field(stroke_paint::MITER_LIMIT, DEFAULT_MITER_LIMIT)
            .map_err(|_| malformed())? as f64,
        dash_array: t
            .f32_vector(stroke_paint::DASH_ARRAY)
            .map_err(|_| malformed())?
            .unwrap_or_default()
            .into_iter()
            .map(f64::from)
            .collect(),
        dash_offset: t.f32_field(stroke_paint::DASH_OFFSET, 0.0).map_err(|_| malformed())? as f64,
    }))
}

fn decode_stops(t: &Table<'_>, slot: u16, layer_id: Uuid) -> Result<Vec<ColorStop>, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };
    let tables = t.table_vector(slot).map_err(|_| malformed())?.unwrap_or_default();
    tables
        .iter()
        .map(|s| {
            Ok(ColorStop {
                offset: s.f32_field(color_stop::OFFSET, 0.0).map_err(|_| malformed())?,
                color: Color::new(
                    s.f32_field(color_stop::R, 0.0).map_err(|_| malformed())?,
                    s.f32_field(color_stop::G, 0.0).map_err(|_| malformed())?,
                    s.f32_field(color_stop::B, 0.0).map_err(|_| malformed())?,
                    s.f32_field(color_stop::A, 0.0).map_err(|_| malformed())?,
                ),
            })
        })
        .collect()
}

fn xy(t: &Table<'_>, sx: u16, sy: u16, layer_id: Uuid) -> Result<Point, AifError> {
    let malformed = || AifError::MalformedPathData { layer_id };
    Ok(Point::new(
        t.f32_field(sx, 0.0).map_err(|_| malformed())? as f64,
        t.f32_field(sy, 0.0).map_err(|_| malformed())? as f64,
    ))
}

// ── Enum mapping (inverse of `encode`) ────────────────────────────────────────

fn fill_rule_from(b: i8) -> FillRule {
    match b {
        1 => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    }
}

fn cap_from(b: i8) -> LineCap {
    match b {
        1 => LineCap::Round,
        2 => LineCap::Square,
        _ => LineCap::Butt,
    }
}

fn join_from(b: i8) -> LineJoin {
    match b {
        1 => LineJoin::Round,
        2 => LineJoin::Bevel,
        _ => LineJoin::Miter,
    }
}

fn spread_from(b: i8) -> SpreadMode {
    match b {
        1 => SpreadMode::Reflect,
        2 => SpreadMode::Repeat,
        _ => SpreadMode::Pad,
    }
}
