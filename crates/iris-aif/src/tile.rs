// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! EXR tile encode/decode for `iris/layers/{id}/tiles/{tx}_{ty}.exr`.
//!
//! Layout: [`TileData`] is interleaved f16 RGBA bytes
//! `[R0,G0,B0,A0, R1,G1,B1,A1, …]`. The exr crate's RGBA channel helper
//! handles the planar/alphabetical channel reordering (A, B, G, R) internally.
//!
//! Sparse-tile optimisation: callers should test [`TileData::is_fully_transparent`]
//! before calling `write_tile_exr` and skip fully-transparent tiles entirely (§4.3).

use std::io::Cursor;

use exr::prelude::{
    f16, AttributeValue, Encoding, Image, Layer, LayerAttributes, ReadChannels,
    SpecificChannels, Text, WritableImage,
};
use exr::image::read::image::ReadLayers;
use uuid::Uuid;

use crate::error::AifError;
use iris_pixel::{TileData, TILE_SIZE};

const TILE_W: usize = TILE_SIZE as usize;
const BPP: usize = 8; // bytes per pixel: 4 channels × 2 (f16)

// ── Write ─────────────────────────────────────────────────────────────────────

/// Encode a [`TileData`] as an in-memory EXR file with `aif:*` sidecar attributes.
///
/// Returns the raw EXR bytes ready to store as an OPC part.
/// Callers **must** have already filtered out fully-transparent tiles.
pub(crate) fn write_tile_exr(
    data: &TileData,
    layer_id: Uuid,
    tx: u32,
    ty: u32,
) -> Result<Vec<u8>, AifError> {
    let bytes: &[u8] = &data.0;

    let get_pixel = |exr::prelude::Vec2(x, y): exr::prelude::Vec2<usize>| {
        let base = (y * TILE_W + x) * BPP;
        let r = f16::from_bits(u16::from_le_bytes([bytes[base], bytes[base + 1]]));
        let g = f16::from_bits(u16::from_le_bytes([bytes[base + 2], bytes[base + 3]]));
        let b = f16::from_bits(u16::from_le_bytes([bytes[base + 4], bytes[base + 5]]));
        let a = f16::from_bits(u16::from_le_bytes([bytes[base + 6], bytes[base + 7]]));
        (r, g, b, a)
    };

    let mut attrs = LayerAttributes::named(Text::from("rgba"));
    attrs.other.insert(
        Text::from("aifLayerId"),
        AttributeValue::Text(Text::from(layer_id.to_string().as_str())),
    );
    attrs.other.insert(Text::from("aifTileX"), AttributeValue::I32(tx as i32));
    attrs.other.insert(Text::from("aifTileY"), AttributeValue::I32(ty as i32));

    let layer = Layer::new(
        (TILE_W, TILE_W),
        attrs,
        Encoding::FAST_LOSSLESS,
        SpecificChannels::rgba(get_pixel),
    );
    let image = Image::from_layer(layer);

    let mut buf = Cursor::new(Vec::new());
    image.write().to_buffered(&mut buf).map_err(|e: exr::error::Error| AifError::ExrEncode {
        layer_id,
        tx,
        ty,
        message: e.to_string(),
    })?;

    Ok(buf.into_inner())
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Decode an EXR tile part back to interleaved f16 RGBA [`TileData`].
///
/// Validates `aifLayerId` against `expected_layer_id`; returns
/// [`AifError::TileMetadataMismatch`] on mismatch. Missing attribute is
/// treated as a recoverable warning (§4.16 rule 4).
pub(crate) fn read_tile_exr(
    bytes: &[u8],
    expected_layer_id: Uuid,
    tx: u32,
    ty: u32,
) -> Result<TileData, AifError> {
    let cursor = Cursor::new(bytes);

    let image = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |_res, _| vec![0u8; TILE_W * TILE_W * BPP],
            |pixels: &mut Vec<u8>, pos, (r, g, b, a): (f16, f16, f16, f16)| {
                let base = (pos.y() * TILE_W + pos.x()) * BPP;
                let rb = r.to_bits().to_le_bytes();
                let gb = g.to_bits().to_le_bytes();
                let bb = b.to_bits().to_le_bytes();
                let ab = a.to_bits().to_le_bytes();
                pixels[base] = rb[0];
                pixels[base + 1] = rb[1];
                pixels[base + 2] = gb[0];
                pixels[base + 3] = gb[1];
                pixels[base + 4] = bb[0];
                pixels[base + 5] = bb[1];
                pixels[base + 6] = ab[0];
                pixels[base + 7] = ab[1];
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(cursor)
        .map_err(|e: exr::error::Error| AifError::TileReadError {
            layer_id: expected_layer_id,
            tx,
            ty,
            message: e.to_string(),
        })?;

    validate_tile_attrs(&image.layer_data.attributes, expected_layer_id, tx, ty)?;

    Ok(TileData(
        image.layer_data.channel_data.pixels.into_boxed_slice(),
    ))
}

fn validate_tile_attrs(
    attrs: &LayerAttributes,
    expected_id: Uuid,
    tx: u32,
    ty: u32,
) -> Result<(), AifError> {
    let id_key = Text::from("aifLayerId");
    if let Some(AttributeValue::Text(id_text)) = attrs.other.get(&id_key) {
        let id_str: String = id_text.clone().into();
        let found = Uuid::parse_str(&id_str).map_err(|_| AifError::TileReadError {
            layer_id: expected_id,
            tx,
            ty,
            message: format!("invalid aifLayerId in EXR: {id_str}"),
        })?;
        if found != expected_id {
            return Err(AifError::TileMetadataMismatch {
                layer_id: expected_id,
                tx,
                ty,
                expected_id,
            });
        }
    }
    // Missing aif:layerId is tolerated per §4.16 rule 4.
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard_tile() -> TileData {
        let mut bytes = vec![0u8; TILE_W * TILE_W * BPP];
        for y in 0..TILE_W {
            for x in 0..TILE_W {
                let base = (y * TILE_W + x) * BPP;
                if (x + y) % 2 == 0 {
                    // opaque white
                    let one = f16::ONE.to_bits().to_le_bytes();
                    bytes[base] = one[0];
                    bytes[base + 1] = one[1];
                    bytes[base + 2] = one[0];
                    bytes[base + 3] = one[1];
                    bytes[base + 4] = one[0];
                    bytes[base + 5] = one[1];
                    bytes[base + 6] = one[0];
                    bytes[base + 7] = one[1];
                }
                // else: transparent black (all zeros)
            }
        }
        TileData(bytes.into_boxed_slice())
    }

    #[test]
    fn round_trip_preserves_pixels() {
        let id = Uuid::nil();
        let original = checkerboard_tile();
        let exr_bytes = write_tile_exr(&original, id, 0, 0).expect("write");
        let recovered = read_tile_exr(&exr_bytes, id, 0, 0).expect("read");
        assert_eq!(original.0.len(), recovered.0.len());
        assert_eq!(&*original.0, &*recovered.0);
    }

    #[test]
    fn layer_id_mismatch_returns_error() {
        let id_a = Uuid::nil();
        let id_b = Uuid::from_u128(1);
        let tile = checkerboard_tile();
        let exr_bytes = write_tile_exr(&tile, id_a, 0, 0).expect("write");
        let err = read_tile_exr(&exr_bytes, id_b, 0, 0).expect_err("should mismatch");
        assert!(matches!(err, AifError::TileMetadataMismatch { .. }));
    }
}
