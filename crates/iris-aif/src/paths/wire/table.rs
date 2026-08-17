// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Table and vector access over a FlatBuffers buffer.
//!
//! Field accessors take a zero-based schema slot number (see
//! [`super::super::schema`]) and fall back to the schema default when the field
//! is absent, matching FlatBuffers' default-elision behaviour.

use super::scalar::{
    follow_uoffset, read_f32, read_i32, read_u16, read_u32, read_u8, slice_at,
};
use super::{Result, WireError};

/// A borrowed view of one FlatBuffers table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Table<'a> {
    buf: &'a [u8],
    /// Absolute position of the table's `soffset`.
    pos: usize,
    /// Absolute position of the table's vtable.
    vtable: usize,
    /// Length of the vtable in bytes.
    vtable_len: usize,
}

impl<'a> Table<'a> {
    /// Build a table view rooted at absolute position `pos`.
    pub(crate) fn new(buf: &'a [u8], pos: usize) -> Result<Self> {
        let soffset = read_i32(buf, pos)?;
        // vtable = pos - soffset, computed in signed space to reject wraparound.
        let vtable = i64::try_from(pos)
            .ok()
            .and_then(|p| p.checked_sub(i64::from(soffset)))
            .and_then(|v| usize::try_from(v).ok())
            .ok_or(WireError::BadOffset { offset: pos })?;
        let vtable_len = read_u16(buf, vtable)? as usize;
        if vtable_len < 4 || vtable.saturating_add(vtable_len) > buf.len() {
            return Err(WireError::BadOffset { offset: vtable });
        }
        Ok(Self { buf, pos, vtable, vtable_len })
    }

    /// Absolute position of field `slot`'s data, or `None` when the field is
    /// absent (short vtable or zero voffset).
    fn field(&self, slot: u16) -> Result<Option<usize>> {
        let vt_off = 4usize.saturating_add(2 * usize::from(slot));
        if vt_off.saturating_add(2) > self.vtable_len {
            return Ok(None);
        }
        let voffset = read_u16(self.buf, self.vtable + vt_off)? as usize;
        if voffset == 0 {
            return Ok(None);
        }
        let at = self.pos.checked_add(voffset).ok_or(WireError::BadOffset { offset: self.pos })?;
        Ok(Some(at))
    }

    /// Read a `ubyte` field, falling back to the schema default.
    pub(crate) fn u8_field(&self, slot: u16, default: u8) -> Result<u8> {
        match self.field(slot)? {
            Some(at) => read_u8(self.buf, at),
            None => Ok(default),
        }
    }

    /// Read a `byte` (signed) field, falling back to the schema default.
    pub(crate) fn i8_field(&self, slot: u16, default: i8) -> Result<i8> {
        Ok(self.u8_field(slot, default as u8)? as i8)
    }

    /// Read a `bool` field, falling back to the schema default.
    pub(crate) fn bool_field(&self, slot: u16, default: bool) -> Result<bool> {
        Ok(self.u8_field(slot, u8::from(default))? != 0)
    }

    /// Read a `uint` field, falling back to the schema default.
    pub(crate) fn u32_field(&self, slot: u16, default: u32) -> Result<u32> {
        match self.field(slot)? {
            Some(at) => read_u32(self.buf, at),
            None => Ok(default),
        }
    }

    /// Read a `float` field, falling back to the schema default.
    pub(crate) fn f32_field(&self, slot: u16, default: f32) -> Result<f32> {
        match self.field(slot)? {
            Some(at) => read_f32(self.buf, at),
            None => Ok(default),
        }
    }

    /// Read a nested table field.
    pub(crate) fn table_field(&self, slot: u16) -> Result<Option<Table<'a>>> {
        match self.field(slot)? {
            Some(at) => Ok(Some(Table::new(self.buf, follow_uoffset(self.buf, at)?)?)),
            None => Ok(None),
        }
    }

    /// Read a `string` field.
    pub(crate) fn string_field(&self, slot: u16) -> Result<Option<&'a str>> {
        let Some(at) = self.field(slot)? else { return Ok(None) };
        let start = follow_uoffset(self.buf, at)?;
        let len = read_u32(self.buf, start)? as usize;
        let bytes = slice_at(self.buf, start + 4, len)?;
        core::str::from_utf8(bytes).map(Some).map_err(|_| WireError::InvalidUtf8)
    }

    /// Element count and first-element position of a vector field.
    fn vector_parts(&self, slot: u16) -> Result<Option<(usize, usize)>> {
        let Some(at) = self.field(slot)? else { return Ok(None) };
        let start = follow_uoffset(self.buf, at)?;
        let len = read_u32(self.buf, start)? as usize;
        Ok(Some((len, start + 4)))
    }

    /// Read a `[ubyte]` / `[byte]` vector as raw bytes.
    pub(crate) fn bytes_vector(&self, slot: u16) -> Result<Option<&'a [u8]>> {
        match self.vector_parts(slot)? {
            Some((len, first)) => Ok(Some(slice_at(self.buf, first, len)?)),
            None => Ok(None),
        }
    }

    /// Read a `[float]` vector.
    pub(crate) fn f32_vector(&self, slot: u16) -> Result<Option<Vec<f32>>> {
        let Some((len, first)) = self.vector_parts(slot)? else { return Ok(None) };
        // Validate the whole span up front so a truncated buffer fails once
        // rather than part-way through the loop (and so the `with_capacity`
        // below cannot be driven by an attacker-chosen length).
        slice_at(self.buf, first, len.saturating_mul(4))?;
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            out.push(read_f32(self.buf, first + i * 4)?);
        }
        Ok(Some(out))
    }

    /// Read a vector of tables.
    pub(crate) fn table_vector(&self, slot: u16) -> Result<Option<Vec<Table<'a>>>> {
        let Some((len, first)) = self.vector_parts(slot)? else { return Ok(None) };
        slice_at(self.buf, first, len.saturating_mul(4))?;
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            out.push(Table::new(self.buf, follow_uoffset(self.buf, first + i * 4)?)?);
        }
        Ok(Some(out))
    }
}

/// Resolve the root table, verifying the file identifier at bytes 4..8.
pub(crate) fn root<'a>(buf: &'a [u8], identifier: &'static str) -> Result<Table<'a>> {
    let ident = slice_at(buf, 4, 4)?;
    if ident != identifier.as_bytes() {
        return Err(WireError::BadFileIdentifier { expected: identifier });
    }
    Table::new(buf, follow_uoffset(buf, 0)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flatbuffers::FlatBufferBuilder;

    /// Build `table { slot0: u32, slot1: string }` so the reader is exercised
    /// against bytes produced by the real upstream builder.
    fn sample() -> Vec<u8> {
        let mut b = FlatBufferBuilder::new();
        let s = b.create_string("hello");
        let t = b.start_table();
        b.push_slot::<u32>(4, 7, 0);
        b.push_slot_always::<flatbuffers::WIPOffset<&str>>(6, s);
        let root = b.end_table(t);
        b.finish(root, Some("TEST"));
        b.finished_data().to_vec()
    }

    #[test]
    fn reads_scalar_and_string_fields() {
        let buf = sample();
        let t = root(&buf, "TEST").expect("root");
        assert_eq!(t.u32_field(0, 0).expect("u32"), 7);
        assert_eq!(t.string_field(1).expect("str"), Some("hello"));
    }

    #[test]
    fn absent_field_yields_default() {
        let buf = sample();
        let t = root(&buf, "TEST").expect("root");
        assert_eq!(t.u32_field(9, 42).expect("default"), 42);
        assert_eq!(t.string_field(9).expect("absent"), None);
        assert!(t.bool_field(9, true).expect("default bool"));
    }

    #[test]
    fn wrong_identifier_is_rejected() {
        let buf = sample();
        assert!(matches!(root(&buf, "AIRF"), Err(WireError::BadFileIdentifier { .. })));
    }

    #[test]
    fn truncated_buffer_errors_instead_of_panicking() {
        let buf = sample();
        for cut in 1..buf.len() {
            // Any prefix must produce an error or a table whose reads error —
            // never a panic.
            if let Ok(t) = root(&buf[..cut], "TEST") {
                let _ = t.u32_field(0, 0);
                let _ = t.string_field(1);
                let _ = t.f32_vector(0);
                let _ = t.table_vector(1);
            }
        }
    }

    #[test]
    fn empty_buffer_errors() {
        assert!(root(&[], "TEST").is_err());
    }
}
