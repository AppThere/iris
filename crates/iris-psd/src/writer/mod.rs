// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! PSD writer: serialise an [`iris_aif::AifDocument`] to a Photoshop `.psd`.
//!
//! Writes 8-bit RGBA pixel layers, group hierarchy (via section-divider
//! records), and a flattened merged composite for maximum reader compatibility.

use std::path::Path;

use iris_aif::{flatten_to_rgba8, AifDocument};

use crate::error::PsdError;

mod records;

/// PSD's maximum dimension per side (PSB raises this to 300,000).
const PSD_MAX_DIM: u32 = 30_000;
/// Number of channels written in the merged image (RGBA).
const MERGED_CHANNELS: u16 = 4;

// ── Big-endian byte helpers (shared with the records/composite submodules) ────

pub(super) fn be16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}
pub(super) fn be32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}
pub(super) fn bei32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_be_bytes());
}
/// Append a Pascal layer name padded so `1 + len` is a multiple of 4.
pub(super) fn pascal_name(buf: &mut Vec<u8>, name: &str) {
    let bytes = name.as_bytes();
    let len = bytes.len().min(255);
    buf.push(len as u8);
    buf.extend_from_slice(&bytes[..len]);
    let pad = (4 - ((len + 1) % 4)) % 4;
    buf.extend(std::iter::repeat(0u8).take(pad));
}

/// Stateless writer that serialises an [`AifDocument`] to PSD bytes.
pub struct PsdWriter;

impl PsdWriter {
    /// Serialise `doc` to a `.psd` file on disk.
    pub fn write(path: &Path, doc: &AifDocument) -> Result<(), PsdError> {
        let bytes = Self::to_bytes(doc)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Serialise `doc` to PSD bytes in memory.
    pub fn to_bytes(doc: &AifDocument) -> Result<Vec<u8>, PsdError> {
        let w = doc.layers.canvas_width;
        let h = doc.layers.canvas_height;
        if w > PSD_MAX_DIM || h > PSD_MAX_DIM {
            return Err(PsdError::DimensionsTooLarge { width: w, height: h });
        }

        let mut buf = file_header(w, h);
        be32(&mut buf, 0); // color mode data section (empty)
        // Image resources: a single ResolutionInfo (0x03ED) block carrying dpi.
        let resources = crate::resources::resolution_resource(doc.layers.dpi_x, doc.layers.dpi_y);
        be32(&mut buf, resources.len() as u32);
        buf.extend_from_slice(&resources);

        records::write_layer_and_mask_section(&mut buf, &doc.layers);

        let merged = flatten_to_rgba8(&doc.layers, w, h);
        write_image_data(&mut buf, &merged, w, h);

        Ok(buf)
    }
}

/// 26-byte PSD file header (version 1, 8-bit, RGB colour mode, RGBA channels).
fn file_header(width: u32, height: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(26);
    h.extend_from_slice(b"8BPS");
    be16(&mut h, 1); // version 1 (PSD)
    h.extend_from_slice(&[0u8; 6]); // reserved
    be16(&mut h, MERGED_CHANNELS);
    be32(&mut h, height);
    be32(&mut h, width);
    be16(&mut h, 8); // depth
    be16(&mut h, 3); // colour mode: RGB
    h
}

/// Append the image-data section: raw planar R, G, B, A for the merged image.
fn write_image_data(buf: &mut Vec<u8>, merged: &[u8], width: u32, height: u32) {
    be16(buf, 0); // raw compression
    let px = (width * height) as usize;
    for channel in 0..4 {
        for i in 0..px {
            buf.push(merged.get(i * 4 + channel).copied().unwrap_or(0));
        }
    }
}
