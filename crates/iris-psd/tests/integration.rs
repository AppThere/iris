// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `iris-psd`.
//!
//! Fixtures are constructed in-memory rather than checked in as binary blobs so
//! the exact byte layout is auditable here. The single-layer, grouped, and
//! empty-layers (composite-fallback) paths are all exercised.

use iris_pixel::LayerContent;
use iris_psd::{PsdError, PsdReader};

// ── Byte helpers (PSD is big-endian) ──────────────────────────────────────────

fn be16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}
fn be32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}
fn bei32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// Append a Pascal layer name padded so `1 + len` is a multiple of 4.
fn pascal_name(buf: &mut Vec<u8>, name: &[u8]) {
    buf.push(name.len() as u8);
    buf.extend_from_slice(name);
    let pad = (4 - ((name.len() + 1) % 4)) % 4;
    buf.extend(std::iter::repeat(0u8).take(pad));
}

/// 26-byte PSD file header for an 8-bit document.
fn header(channels: u16, width: u32, height: u32, color_mode: u16) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(b"8BPS");
    be16(&mut h, 1); // version 1 (PSD)
    h.extend_from_slice(&[0u8; 6]); // reserved
    be16(&mut h, channels);
    be32(&mut h, height);
    be32(&mut h, width);
    be16(&mut h, 8); // depth
    be16(&mut h, color_mode);
    h
}

/// Raw image-data section: one `0` compression marker then `channels` planes.
fn raw_image_data(channels: usize, width: u32, height: u32, fill: u8) -> Vec<u8> {
    let mut d = Vec::new();
    be16(&mut d, 0); // raw compression
    d.extend(std::iter::repeat(fill).take(channels * (width * height) as usize));
    d
}

/// A 2×2 RGBA content layer record plus its channel image data (opaque red).
fn content_layer(name: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let plane = 4usize; // 2×2
    let mut r = Vec::new();
    bei32(&mut r, 0); // top
    bei32(&mut r, 0); // left
    bei32(&mut r, 2); // bottom (parser subtracts 1)
    bei32(&mut r, 2); // right
    be16(&mut r, 4); // 4 channels
    for id in [0i16, 1, 2, -1] {
        r.extend_from_slice(&id.to_be_bytes());
        be32(&mut r, (2 + plane) as u32);
    }
    r.extend_from_slice(b"8BIM");
    r.extend_from_slice(b"norm");
    r.push(255); // opacity
    r.push(1); // clipping: non-base
    r.push(0b0000_0010); // flags: visible
    r.push(0); // filler
    be32(&mut r, (4 + 4 + 1 + name.len()) as u32); // extra-data length
    be32(&mut r, 0); // mask data length
    be32(&mut r, 0); // blending ranges length
    pascal_name(&mut r, name);

    let mut data = Vec::new();
    for fill in [255u8, 0, 0, 255] {
        be16(&mut data, 0);
        data.extend(std::iter::repeat(fill).take(plane));
    }
    (r, data)
}

/// A section-divider record (no channels) carrying an `lsct` divider type.
/// `divider_type`: 1 = open folder, 3 = bounding section.
fn divider_layer(name: &[u8], divider_type: i32) -> Vec<u8> {
    let mut lsct = Vec::new();
    lsct.extend_from_slice(b"8BIM");
    lsct.extend_from_slice(b"lsct");
    be32(&mut lsct, 4);
    bei32(&mut lsct, divider_type);

    let mut name_field = Vec::new();
    pascal_name(&mut name_field, name);

    let mut r = Vec::new();
    bei32(&mut r, 0);
    bei32(&mut r, 0);
    bei32(&mut r, 0);
    bei32(&mut r, 0);
    be16(&mut r, 0); // 0 channels
    r.extend_from_slice(b"8BIM");
    r.extend_from_slice(b"norm");
    r.push(255);
    r.push(1);
    r.push(0b0000_0010); // visible
    r.push(0);
    be32(&mut r, (4 + 4 + name_field.len() + lsct.len()) as u32);
    be32(&mut r, 0); // mask data length
    be32(&mut r, 0); // blending ranges length
    r.extend_from_slice(&name_field);
    r.extend_from_slice(&lsct);
    r
}

/// Assemble a full PSD from layer records and their channel data (both in
/// bottom-to-top file order).
fn assemble(channels: u16, w: u32, h: u32, records: &[Vec<u8>], channel_data: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    be16(&mut body, records.len() as u16); // layer count
    for r in records {
        body.extend_from_slice(r);
    }
    for d in channel_data {
        body.extend_from_slice(d);
    }

    let mut section = Vec::new();
    be32(&mut section, body.len() as u32); // layer info length
    section.extend_from_slice(&body);
    be32(&mut section, 0); // global layer mask info length

    let mut buf = header(channels, w, h, 3);
    be32(&mut buf, 0); // color mode data
    be32(&mut buf, 0); // image resources
    be32(&mut buf, section.len() as u32);
    buf.extend_from_slice(&section);
    buf.extend(raw_image_data(channels as usize, w, h, 0));
    buf
}

// ── Fixtures ────────────────────────────────────────────────────────────────

fn single_layer_psd() -> Vec<u8> {
    let (record, data) = content_layer(b"Layer 1");
    assemble(4, 2, 2, &[record], &[data])
}

fn grouped_psd() -> Vec<u8> {
    // File order, bottom to top: bounding-section divider, content, open-folder.
    let bound = divider_layer(b"</Layer group>", 3);
    let (content, content_data) = content_layer(b"Layer 1");
    let open = divider_layer(b"Group A", 1);
    assemble(
        4,
        2,
        2,
        &[bound, content, open],
        &[Vec::new(), content_data, Vec::new()],
    )
}

fn no_layer_psd(width: u32, height: u32) -> Vec<u8> {
    let mut buf = header(3, width, height, 3); // RGB
    be32(&mut buf, 0); // color mode data
    be32(&mut buf, 0); // image resources
    be32(&mut buf, 0); // empty layer and mask section
    buf.extend(raw_image_data(3, width, height, 200));
    buf
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn reads_single_layer_name_count_and_dimensions() {
    let doc = PsdReader::from_bytes(&single_layer_psd()).expect("PSD should parse");

    assert_eq!(doc.canvas.width_px, 2);
    assert_eq!(doc.canvas.height_px, 2);

    let root = doc.layers.root_layer_ids();
    assert_eq!(root.len(), 1, "exactly one root layer");

    let layer = doc.layers.get(root[0]).expect("layer present");
    assert_eq!(layer.name, "Layer 1");
    assert!(layer.visible);
    assert!((layer.opacity - 1.0).abs() < 1e-6);
    assert!(!layer.clipping_mask);
    assert!(matches!(layer.content, LayerContent::Pixel(_)));
}

#[test]
fn reads_group_hierarchy() {
    let doc = PsdReader::from_bytes(&grouped_psd()).expect("grouped PSD should parse");

    let root = doc.layers.root_layer_ids();
    assert_eq!(root.len(), 1, "the group is the only root node");

    let group = doc.layers.get(root[0]).expect("group present");
    assert_eq!(group.name, "Group A");
    let LayerContent::Group { ref children } = group.content else {
        panic!("root node should be a group, got {:?}", group.content);
    };
    assert_eq!(children.len(), 1, "group has one child layer");

    let child = doc.layers.get(children[0]).expect("child present");
    assert_eq!(child.name, "Layer 1");
    assert!(matches!(child.content, LayerContent::Pixel(_)));
}

#[test]
fn reads_no_layer_psd_via_composite_fallback() {
    // COMPAT(adobe): a PSD with an empty layer section still has pixels in the
    // merged image-data section; we synthesise a single "Background" layer.
    let doc = PsdReader::from_bytes(&no_layer_psd(4, 3)).expect("PSD should parse");

    assert_eq!(doc.canvas.width_px, 4);
    assert_eq!(doc.canvas.height_px, 3);

    let root = doc.layers.root_layer_ids();
    assert_eq!(root.len(), 1);
    let layer = doc.layers.get(root[0]).expect("layer present");
    assert_eq!(layer.name, "Background");
    assert!(matches!(layer.content, LayerContent::Pixel(_)));
}

#[test]
fn rejects_non_psd_file() {
    let png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(matches!(PsdReader::from_bytes(&png), Err(PsdError::BadSignature)));
}
