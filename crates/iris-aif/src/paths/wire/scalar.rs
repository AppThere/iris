// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Checked primitive reads over a FlatBuffers buffer.
//!
//! Every function here validates its span against the buffer length before
//! decoding, so a truncated or hostile `paths.bin` produces a [`WireError`]
//! rather than an out-of-bounds panic.

use super::{Result, WireError};

/// Borrow `len` bytes starting at `offset`, or fail.
pub(super) fn slice_at(buf: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or(WireError::OutOfBounds { offset })?;
    buf.get(offset..end).ok_or(WireError::OutOfBounds { offset })
}

/// Read a `ubyte`.
pub(super) fn read_u8(buf: &[u8], offset: usize) -> Result<u8> {
    buf.get(offset).copied().ok_or(WireError::OutOfBounds { offset })
}

/// Read a little-endian `ushort`.
pub(super) fn read_u16(buf: &[u8], offset: usize) -> Result<u16> {
    let b = slice_at(buf, offset, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

/// Read a little-endian `uint`.
pub(super) fn read_u32(buf: &[u8], offset: usize) -> Result<u32> {
    let b = slice_at(buf, offset, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Read a little-endian `int` (used for a table's `soffset`).
pub(super) fn read_i32(buf: &[u8], offset: usize) -> Result<i32> {
    Ok(read_u32(buf, offset)? as i32)
}

/// Read a little-endian `float`.
pub(super) fn read_f32(buf: &[u8], offset: usize) -> Result<f32> {
    Ok(f32::from_bits(read_u32(buf, offset)?))
}

/// Resolve a `uoffset` stored at `offset` into an absolute buffer position.
///
/// FlatBuffers `uoffset`s are relative to the position of the offset itself and
/// always point forward, so the target must land inside the buffer.
pub(super) fn follow_uoffset(buf: &[u8], offset: usize) -> Result<usize> {
    let rel = read_u32(buf, offset)? as usize;
    let target = offset.checked_add(rel).ok_or(WireError::BadOffset { offset })?;
    if target >= buf.len() {
        return Err(WireError::BadOffset { offset });
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_scalars() {
        let buf = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(read_u8(&buf, 0).expect("u8"), 1);
        assert_eq!(read_u16(&buf, 0).expect("u16"), 0x0201);
        assert_eq!(read_u32(&buf, 0).expect("u32"), 0x04030201);
    }

    #[test]
    fn reads_past_end_are_errors_not_panics() {
        let buf = [0u8; 3];
        assert!(read_u32(&buf, 0).is_err());
        assert!(read_u16(&buf, 2).is_err());
        assert!(read_u8(&buf, 3).is_err());
        assert!(slice_at(&buf, 1, 9).is_err());
    }

    #[test]
    fn offset_overflow_is_rejected() {
        let buf = [0u8; 8];
        assert!(slice_at(&buf, usize::MAX, 1).is_err());
    }

    #[test]
    fn uoffset_pointing_past_the_buffer_is_rejected() {
        // A uoffset of 0xFFFF_FFFF from position 0 cannot land inside 8 bytes.
        let buf = [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];
        assert!(matches!(follow_uoffset(&buf, 0), Err(WireError::BadOffset { .. })));
    }

    #[test]
    fn f32_round_trips_through_bit_pattern() {
        let bytes = 1.5f32.to_le_bytes();
        assert_eq!(read_f32(&bytes, 0).expect("f32"), 1.5);
    }
}
