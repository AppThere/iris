// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Colour-conversion helpers for the CPU composite path: f16 decode and a
//! linear-light → sRGB lookup table. Split out of `cpu.rs` to keep both files
//! within the 300-line ceiling.

use std::sync::OnceLock;

/// Convert IEEE 754 half-precision bits to f32.
/// Subnormal f16 values (exp=0, mant≠0) are treated as zero — adequate for pixels.
pub(crate) fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp = ((bits as u32) & 0x7C00) >> 10;
    let mant = (bits as u32) & 0x03FF;
    f32::from_bits(match exp {
        0 => sign,                               // ±zero or subnormal → zero
        31 => sign | 0x7F80_0000 | (mant << 13), // ±inf / NaN pass-through
        e => sign | ((e + 112) << 23) | (mant << 13), // normal: rebias 15→127
    })
}

const LUT_SIZE: usize = 4096;

pub(crate) fn lut_index(linear: f32) -> usize {
    (linear.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32) as usize
}

/// Linear-light → sRGB u8 lookup table. `powf` per channel per pixel was the
/// dominant cost of the conversion loop (~10M `powf` per frame at 1080p).
pub(crate) fn srgb_lut() -> &'static [u8; LUT_SIZE] {
    static LUT: OnceLock<[u8; LUT_SIZE]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut lut = [0u8; LUT_SIZE];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = to_srgb_u8(i as f32 / (LUT_SIZE - 1) as f32);
        }
        lut
    })
}

/// Encode a linear-light [0,1] value to sRGB gamma and clamp to u8.
fn to_srgb_u8(linear: f32) -> u8 {
    let s = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.clamp(0.0, 1.0).powf(1.0 / 2.4) - 0.055
    };
    (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_decode_basics() {
        assert_eq!(f16_to_f32(0x0000), 0.0); // +0
        assert_eq!(f16_to_f32(0x3C00), 1.0); // 1.0
        assert_eq!(f16_to_f32(0x4000), 2.0); // 2.0
    }

    #[test]
    fn srgb_lut_endpoints() {
        let lut = srgb_lut();
        assert_eq!(lut[lut_index(0.0)], 0);
        assert_eq!(lut[lut_index(1.0)], 255);
    }
}
