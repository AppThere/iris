// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `iris-psd`.
//!
//! Fixtures are constructed in-memory rather than checked in as binary blobs so
//! the exact byte layout is auditable here. Both the real layer-record path and
//! the empty-layers composite fallback are exercised.

use iris_pixel::LayerContent;
use iris_psd::{PsdError, PsdReader};

// ── Byte helpers (PSD is big-endian) ──────────────────────────────────────────

fn push_be16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}
fn push_be32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}
fn push_bei32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// Build the 26-byte PSD file header for an 8-bit document.
fn header(channels: u16, width: u32, height: u32, color_mode: u16) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(b"8BPS");
    push_be16(&mut h, 1); // version 1 (PSD)
    h.extend_from_slice(&[0u8; 6]); // reserved
    push_be16(&mut h, channels);
    push_be32(&mut h, height);
    push_be32(&mut h, width);
    push_be16(&mut h, 8); // depth
    push_be16(&mut h, color_mode);
    h
}

/// Raw image-data section: one `0` compression marker then `channels` planes of
/// `width * height` bytes each.
fn raw_image_data(channels: usize, width: u32, height: u32, fill: u8) -> Vec<u8> {
    let mut d = Vec::new();
    push_be16(&mut d, 0); // raw compression
    d.extend(std::iter::repeat(fill).take(channels * (width * height) as usize));
    d
}

/// A PSD with no explicit layers (empty layer-and-mask section). Pixels live
/// only in the merged image-data section.
fn no_layer_psd(width: u32, height: u32) -> Vec<u8> {
    let mut buf = header(3, width, height, 3); // RGB
    push_be32(&mut buf, 0); // color mode data length
    push_be32(&mut buf, 0); // image resources length
    push_be32(&mut buf, 0); // layer and mask section length (no layers)
    buf.extend(raw_image_data(3, width, height, 200));
    buf
}

/// A single 2×2 RGBA layer named "Layer 1", fully opaque red.
fn single_layer_psd() -> Vec<u8> {
    let (w, h): (u32, u32) = (2, 2);
    let plane = (w * h) as usize; // 4 bytes per channel

    // ── Layer record ─────────────────────────────────────────────────────────
    let mut record = Vec::new();
    push_bei32(&mut record, 0); // top
    push_bei32(&mut record, 0); // left
    push_bei32(&mut record, h as i32); // bottom (parser subtracts 1)
    push_bei32(&mut record, w as i32); // right  (parser subtracts 1)
    push_be16(&mut record, 4); // channel count (RGBA)
    for id in [0i16, 1, 2, -1] {
        record.extend_from_slice(&id.to_be_bytes());
        push_be32(&mut record, (2 + plane) as u32); // 2-byte compression + data
    }
    record.extend_from_slice(b"8BIM"); // blend mode signature
    record.extend_from_slice(b"norm"); // blend mode key = Normal
    record.push(255); // opacity
    record.push(1); // clipping: 1 = non-base (not a clipping mask)
    record.push(0b0000_0010); // flags: bit 1 set = visible
    record.push(0); // filler
    let name = b"Layer 1"; // 7 bytes; (1 + 7) is a multiple of 4 → no padding
    push_be32(&mut record, (4 + 4 + 1 + name.len()) as u32); // extra-data length
    push_be32(&mut record, 0); // layer mask data length
    push_be32(&mut record, 0); // layer blending ranges length
    record.push(name.len() as u8);
    record.extend_from_slice(name);

    // ── Channel image data: comp(0) + 4 bytes, per channel (R,G,B,A) ──────────
    let mut channel_data = Vec::new();
    for fill in [255u8, 0, 0, 255] {
        push_be16(&mut channel_data, 0); // raw
        channel_data.extend(std::iter::repeat(fill).take(plane));
    }

    // ── Layer info body + section wrapper ─────────────────────────────────────
    let mut layer_info_body = Vec::new();
    push_be16(&mut layer_info_body, 1); // layer count
    layer_info_body.extend_from_slice(&record);
    layer_info_body.extend_from_slice(&channel_data);

    let mut section = Vec::new();
    push_be32(&mut section, layer_info_body.len() as u32); // layer info length
    section.extend_from_slice(&layer_info_body);
    push_be32(&mut section, 0); // global layer mask info length

    let mut buf = header(4, w, h, 3); // RGBA, RGB colour mode
    push_be32(&mut buf, 0); // color mode data length
    push_be32(&mut buf, 0); // image resources length
    push_be32(&mut buf, section.len() as u32); // layer and mask section length
    buf.extend_from_slice(&section);
    buf.extend(raw_image_data(4, w, h, 0)); // merged composite (unused here)
    buf
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn reads_single_layer_name_count_and_dimensions() {
    let doc = PsdReader::from_bytes(&single_layer_psd()).expect("PSD should parse");

    assert_eq!(doc.canvas.width_px, 2);
    assert_eq!(doc.canvas.height_px, 2);
    assert_eq!(doc.layers.canvas_width, 2);
    assert_eq!(doc.layers.canvas_height, 2);

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
