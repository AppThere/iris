// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Round-trip and malformed-input tests for the §4.10 path store.

use uuid::Uuid;

use super::{read_path_store, write_path_store};
use crate::error::AifError;
use iris_vector::{
    Affine, BezPath, Color, ColorStop, FillRule, LineCap, LineJoin, LinearGradient, Paint,
    PathObject, Point, RadialGradient, SpreadMode, StrokePaint, VectorLayer,
};

fn sample_path() -> BezPath {
    let mut p = BezPath::new();
    p.move_to(Point::new(1.0, 2.0));
    p.line_to(Point::new(3.0, 4.0));
    p.quad_to(Point::new(5.0, 6.0), Point::new(7.0, 8.0));
    p.curve_to(Point::new(9.0, 10.0), Point::new(11.0, 12.0), Point::new(13.0, 14.0));
    p.close_path();
    p
}

fn sample_layer() -> VectorLayer {
    let mut solid = PathObject::new(sample_path(), Some(Paint::Solid(Color::new(0.25, 0.5, 0.75, 1.0))));
    solid.name = "Solid".into();
    solid.fill_rule = FillRule::EvenOdd;
    solid.transform = Affine::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    solid.stroke = Some(StrokePaint {
        paint: Paint::Solid(Color::new(1.0, 0.0, 0.0, 0.5)),
        width: 2.5,
        cap: LineCap::Round,
        join: LineJoin::Bevel,
        miter_limit: 8.0,
        dash_array: vec![4.0, 2.0],
        dash_offset: 1.0,
    });

    let mut linear = PathObject::new(sample_path(), Some(Paint::Linear(LinearGradient {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 10.0),
        stops: vec![
            ColorStop { offset: 0.0, color: Color::BLACK },
            ColorStop { offset: 1.0, color: Color::new(1.0, 1.0, 1.0, 1.0) },
        ],
        spread: SpreadMode::Reflect,
    })));
    linear.visible = false;

    let radial = PathObject::new(sample_path(), Some(Paint::Radial(RadialGradient {
        center: Point::new(5.0, 5.0),
        focus: Point::new(4.0, 4.0),
        radius: 7.5,
        stops: vec![ColorStop { offset: 0.5, color: Color::new(0.1, 0.2, 0.3, 0.4) }],
        spread: SpreadMode::Repeat,
    })));

    let unpainted = PathObject::new(sample_path(), None);

    VectorLayer { objects: vec![solid, linear, radial, unpainted], color_space: "srgb".into() }
}

fn roundtrip(layer: &VectorLayer) -> VectorLayer {
    let bytes = write_path_store(layer);
    read_path_store(&bytes, Uuid::nil(), None).expect("path store must decode")
}

#[test]
fn file_identifier_is_written() {
    let bytes = write_path_store(&sample_layer());
    assert_eq!(&bytes[4..8], b"AIRF");
}

#[test]
fn roundtrip_preserves_object_count_and_ids() {
    let layer = sample_layer();
    let out = roundtrip(&layer);
    assert_eq!(out.color_space, "srgb");
    assert_eq!(out.objects.len(), layer.objects.len());
    for (a, b) in layer.objects.iter().zip(out.objects.iter()) {
        assert_eq!(a.id, b.id, "object ids must survive the round-trip");
    }
}

#[test]
fn roundtrip_preserves_path_geometry() {
    let layer = sample_layer();
    let out = roundtrip(&layer);
    // `sample_path` uses only integral coordinates, which survive the schema's
    // f32 narrowing exactly, so the verb/point streams must match element-wise.
    assert_eq!(layer.objects[0].path.elements(), out.objects[0].path.elements());
}

#[test]
fn roundtrip_preserves_object_flags_and_transform() {
    let layer = sample_layer();
    let out = roundtrip(&layer);
    assert_eq!(out.objects[0].name, "Solid");
    assert_eq!(out.objects[0].fill_rule, FillRule::EvenOdd);
    assert_eq!(out.objects[0].transform.as_coeffs(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert!(out.objects[0].visible);
    assert!(!out.objects[1].visible, "visible=false must not be lost");
}

#[test]
fn roundtrip_preserves_solid_fill_and_stroke() {
    let out = roundtrip(&sample_layer());
    assert_eq!(out.objects[0].fill, Some(Paint::Solid(Color::new(0.25, 0.5, 0.75, 1.0))));
    let s = out.objects[0].stroke.as_ref().expect("stroke present");
    assert_eq!(s.width, 2.5);
    assert_eq!(s.cap, LineCap::Round);
    assert_eq!(s.join, LineJoin::Bevel);
    assert_eq!(s.miter_limit, 8.0);
    assert_eq!(s.dash_array, vec![4.0, 2.0]);
    assert_eq!(s.dash_offset, 1.0);
}

#[test]
fn roundtrip_preserves_gradients() {
    let out = roundtrip(&sample_layer());
    match out.objects[1].fill.as_ref().expect("linear fill") {
        Paint::Linear(g) => {
            assert_eq!(g.start, Point::new(0.0, 0.0));
            assert_eq!(g.end, Point::new(10.0, 10.0));
            assert_eq!(g.stops.len(), 2);
            assert_eq!(g.spread, SpreadMode::Reflect);
        }
        other => panic!("expected linear gradient, got {other:?}"),
    }
    match out.objects[2].fill.as_ref().expect("radial fill") {
        Paint::Radial(g) => {
            assert_eq!(g.center, Point::new(5.0, 5.0));
            assert_eq!(g.focus, Point::new(4.0, 4.0));
            assert_eq!(g.radius, 7.5);
            assert_eq!(g.spread, SpreadMode::Repeat);
        }
        other => panic!("expected radial gradient, got {other:?}"),
    }
}

#[test]
fn unpainted_object_stays_unpainted() {
    let out = roundtrip(&sample_layer());
    assert!(out.objects[3].fill.is_none());
    assert!(out.objects[3].stroke.is_none());
}

#[test]
fn empty_layer_roundtrips() {
    let layer = VectorLayer::new("srgb");
    let out = roundtrip(&layer);
    assert!(out.objects.is_empty());
}

#[test]
fn color_space_mismatch_is_detected() {
    let bytes = write_path_store(&sample_layer());
    let err = read_path_store(&bytes, Uuid::nil(), Some("display-p3")).expect_err("must mismatch");
    assert!(matches!(err, AifError::ColorSpaceMismatch { .. }), "got {err:?}");
}

#[test]
fn matching_color_space_is_accepted() {
    let bytes = write_path_store(&sample_layer());
    assert!(read_path_store(&bytes, Uuid::nil(), Some("srgb")).is_ok());
}

#[test]
fn bad_file_identifier_is_malformed() {
    let mut bytes = write_path_store(&sample_layer());
    bytes[4] = b'X';
    let err = read_path_store(&bytes, Uuid::nil(), None).expect_err("must reject");
    assert!(matches!(err, AifError::MalformedPathData { .. }), "got {err:?}");
}

#[test]
fn truncated_buffer_never_panics() {
    let bytes = write_path_store(&sample_layer());
    for cut in 0..bytes.len() {
        // Any prefix must produce a typed error rather than unwind.
        let _ = read_path_store(&bytes[..cut], Uuid::nil(), None);
    }
}

#[test]
fn garbage_bytes_never_panic() {
    for len in 0..64usize {
        let junk: Vec<u8> = (0..len).map(|i| (i * 37 % 251) as u8).collect();
        let _ = read_path_store(&junk, Uuid::nil(), None);
    }
}
