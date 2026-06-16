// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Layer-and-mask section serialisation: pixel layer records, group section
//! dividers, and channel image data, in Photoshop's bottom-to-top file order.

use iris_aif::layer_to_rgba8;
use iris_pixel::{Layer, LayerContent, LayerId, LayerTree};

use super::{be16, be32, bei32, pascal_name};
use crate::blend::to_psd_key;

/// Append the complete layer-and-mask information section for `tree`.
pub(super) fn write_layer_and_mask_section(buf: &mut Vec<u8>, tree: &LayerTree) {
    let mut records: Vec<Vec<u8>> = Vec::new();
    let mut channels: Vec<Vec<u8>> = Vec::new();

    // Root layers are stored top-first; PSD file order is bottom-to-top.
    for id in tree.root_layer_ids().iter().rev() {
        emit_node(tree, *id, &mut records, &mut channels);
    }

    if records.is_empty() {
        be32(buf, 0); // no layers: a single zero-length section
        return;
    }

    let mut body = Vec::new();
    be16(&mut body, records.len() as u16); // layer count (positive)
    for r in &records {
        body.extend_from_slice(r);
    }
    for c in &channels {
        body.extend_from_slice(c);
    }

    // section = layer-info length + body + global-layer-mask length (0).
    let section_len = 4 + body.len() + 4;
    be32(buf, section_len as u32);
    be32(buf, body.len() as u32);
    buf.extend_from_slice(&body);
    be32(buf, 0); // global layer mask info length
}

/// Emit a layer (and, for groups, its bounding/open dividers and children) in
/// file order. Groups expand to `[bounding, children…, open(name)]`.
fn emit_node(tree: &LayerTree, id: LayerId, records: &mut Vec<Vec<u8>>, channels: &mut Vec<Vec<u8>>) {
    let Some(layer) = tree.get(id) else {
        return;
    };
    match &layer.content {
        LayerContent::Pixel(_) => {
            if let Some((record, data)) = pixel_record(layer) {
                records.push(record);
                channels.push(data);
            }
        }
        LayerContent::Group { children } => {
            // Bounding-section divider sits below the group's content and carries
            // the group's composite properties (opacity/blend/visibility).
            records.push(divider_record(layer, 3, "</Layer group>"));
            channels.push(Vec::new());
            for child in children.iter().rev() {
                emit_node(tree, *child, records, channels);
            }
            // Open-folder divider sits on top and carries the group name.
            records.push(divider_record(layer, 1, &layer.name));
            channels.push(Vec::new());
        }
        other => {
            // COMPAT(adobe): vector/text/adjustment layers are not yet
            // rasterised on export; skip rather than emit an invalid record.
            tracing::warn!(?other, name = layer.name, "PSD export: skipping unsupported layer");
        }
    }
}

/// Build the record + channel image data for an RGBA pixel layer.
fn pixel_record(layer: &Layer) -> Option<(Vec<u8>, Vec<u8>)> {
    let px = layer_to_rgba8(layer)?;
    let (w, h) = (px.width, px.height);
    let plane = (w * h) as usize;

    let mut record = Vec::new();
    bei32(&mut record, px.offset_y); // top
    bei32(&mut record, px.offset_x); // left
    bei32(&mut record, px.offset_y + h as i32); // bottom (reader subtracts 1)
    bei32(&mut record, px.offset_x + w as i32); // right
    be16(&mut record, 4); // RGBA channels
    for id in [0i16, 1, 2, -1] {
        record.extend_from_slice(&id.to_be_bytes());
        be32(&mut record, (2 + plane) as u32); // 2-byte compression + data
    }
    write_common(&mut record, layer, &layer.name, &[]);
    pascal_name(&mut record, &layer.name);

    // Channel image data: raw planar R, G, B, A.
    let mut data = Vec::new();
    for channel in [0usize, 1, 2, 3] {
        be16(&mut data, 0); // raw compression
        for i in 0..plane {
            data.push(px.rgba.get(i * 4 + channel).copied().unwrap_or(0));
        }
    }
    Some((record, data))
}

/// Build a section-divider record (no channels) with an `lsct` divider type.
/// `divider_type`: 1 = open folder, 3 = bounding section.
fn divider_record(layer: &Layer, divider_type: i32, name: &str) -> Vec<u8> {
    let mut lsct = Vec::new();
    lsct.extend_from_slice(b"8BIM");
    lsct.extend_from_slice(b"lsct");
    be32(&mut lsct, 4);
    bei32(&mut lsct, divider_type);

    let mut record = Vec::new();
    for _ in 0..4 {
        bei32(&mut record, 0); // empty rectangle
    }
    be16(&mut record, 0); // 0 channels
    write_common(&mut record, layer, name, &lsct);
    pascal_name(&mut record, name);
    record.extend_from_slice(&lsct);
    record
}

/// Write the fields shared by every record: blend mode, opacity, clipping,
/// flags, filler, and the extra-data length. `name` is the Pascal name the
/// caller will append next; `trailer` is any additional layer information
/// (e.g. `lsct`) appended after the name.
fn write_common(record: &mut Vec<u8>, layer: &Layer, name: &str, trailer: &[u8]) {
    record.extend_from_slice(b"8BIM");
    record.extend_from_slice(&to_psd_key(layer.blend_mode));
    record.push((layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
    // Reader maps clipping byte 0 -> clipping mask; 1 -> base layer.
    record.push(if layer.clipping_mask { 0 } else { 1 });
    record.push(if layer.visible { 0b0000_0010 } else { 0 });
    record.push(0); // filler

    // extra-data length = mask(4) + blending(4) + name field + trailer.
    let name_field = 1 + name_padded_len(name);
    be32(record, (4 + 4 + name_field + trailer.len()) as u32);
    be32(record, 0); // layer mask data length
    be32(record, 0); // layer blending ranges length
}

/// The padded byte length of a Pascal name (excluding the leading length byte).
fn name_padded_len(name: &str) -> usize {
    let len = name.as_bytes().len().min(255);
    len + (4 - ((len + 1) % 4)) % 4
}
