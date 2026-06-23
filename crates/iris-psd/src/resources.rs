// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Photoshop image-resource block handling for document resolution.
//!
//! The `psd` crate does not surface the resolution, so [`read_resolution`]
//! walks the image-resources section of the raw file and parses the
//! ResolutionInfo block (id `0x03ED`). [`resolution_resource`] builds the same
//! block for the writer. All parsing is bounds-checked and never panics on
//! malformed input.

/// Image-resource id for ResolutionInfo (`0x03ED`).
const RESOLUTION_INFO_ID: u16 = 0x03ED;
/// PSD file header length; the image-resources section follows the colour-mode
/// data section, which starts immediately after the header.
const HEADER_LEN: usize = 26;
/// Fixed-point (16.16) divisor used by PSD resolution values.
const FIXED_ONE: f64 = 65536.0;

/// Read the document's `(dpi_x, dpi_y)` from a PSD's image resources, or `None`
/// if the file has no ResolutionInfo block (or is malformed/truncated).
pub(crate) fn read_resolution(bytes: &[u8]) -> Option<(f32, f32)> {
    // [header][colour-mode data: u32 len + data][image resources: u32 len + data]
    let cmd_len = be_u32(bytes, HEADER_LEN)? as usize;
    let res_off = HEADER_LEN.checked_add(4)?.checked_add(cmd_len)?;
    let res_len = be_u32(bytes, res_off)? as usize;
    let start = res_off.checked_add(4)?;
    let end = start.checked_add(res_len)?;
    find_resolution(bytes.get(start..end)?)
}

/// Walk the resource blocks in the image-resources `section`, returning the
/// resolution from the first ResolutionInfo block.
fn find_resolution(section: &[u8]) -> Option<(f32, f32)> {
    let mut p = 0usize;
    while p + 4 <= section.len() {
        if &section[p..p + 4] != b"8BIM" {
            return None; // corrupt section; stop rather than guess
        }
        p += 4;
        let id = u16::from_be_bytes([*section.get(p)?, *section.get(p + 1)?]);
        p = p.checked_add(2)?;
        // Pascal name: length byte + content, padded so the field length is even.
        let name_field = 1 + *section.get(p)? as usize;
        p = p.checked_add(name_field + (name_field & 1))?;
        let size = be_u32(section, p)? as usize;
        p = p.checked_add(4)?;
        let data = section.get(p..p.checked_add(size)?)?;
        if id == RESOLUTION_INFO_ID {
            return parse_resolution_info(data);
        }
        // Resource data is padded to an even length.
        p = p.checked_add(size + (size & 1))?;
    }
    None
}

/// Parse a 16-byte ResolutionInfo payload: hRes (16.16 fixed) at `[0..4]` and
/// vRes at `[8..12]`, both in pixels per inch.
fn parse_resolution_info(d: &[u8]) -> Option<(f32, f32)> {
    let h = i32::from_be_bytes(d.get(0..4)?.try_into().ok()?);
    let v = i32::from_be_bytes(d.get(8..12)?.try_into().ok()?);
    let dpi_x = h as f64 / FIXED_ONE;
    let dpi_y = v as f64 / FIXED_ONE;
    (dpi_x > 0.0 && dpi_y > 0.0).then_some((dpi_x as f32, dpi_y as f32))
}

/// Build a complete ResolutionInfo (`0x03ED`) image-resource block for the
/// writer: `8BIM` + id + empty (padded) name + size + 16-byte payload.
pub(crate) fn resolution_resource(dpi_x: f32, dpi_y: f32) -> Vec<u8> {
    let fixed = |dpi: f32| ((dpi as f64) * FIXED_ONE).round() as i32;
    let mut b = Vec::with_capacity(28);
    b.extend_from_slice(b"8BIM");
    b.extend_from_slice(&RESOLUTION_INFO_ID.to_be_bytes());
    b.extend_from_slice(&[0u8, 0u8]); // empty Pascal name, padded to even
    b.extend_from_slice(&16u32.to_be_bytes()); // payload size
    b.extend_from_slice(&fixed(dpi_x).to_be_bytes()); // hRes (16.16 fixed)
    b.extend_from_slice(&1u16.to_be_bytes()); // hResUnit: pixels/inch
    b.extend_from_slice(&1u16.to_be_bytes()); // widthUnit: inches
    b.extend_from_slice(&fixed(dpi_y).to_be_bytes()); // vRes
    b.extend_from_slice(&1u16.to_be_bytes()); // vResUnit: pixels/inch
    b.extend_from_slice(&1u16.to_be_bytes()); // heightUnit: inches
    b
}

/// Read a big-endian `u32` at `off`, or `None` if out of bounds.
fn be_u32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off.checked_add(4)?)?;
    Some(u32::from_be_bytes(s.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PSD prefix: 26-byte header, empty colour-mode section,
    /// and an image-resources section holding `resource`.
    fn psd_prefix(resource: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN];
        b.extend_from_slice(&0u32.to_be_bytes()); // colour-mode data length 0
        b.extend_from_slice(&(resource.len() as u32).to_be_bytes());
        b.extend_from_slice(resource);
        b
    }

    #[test]
    fn write_then_read_round_trips() {
        let bytes = psd_prefix(&resolution_resource(150.0, 300.0));
        let (x, y) = read_resolution(&bytes).expect("resolution present");
        assert!((x - 150.0).abs() < 1e-3, "dpi_x {x}");
        assert!((y - 300.0).abs() < 1e-3, "dpi_y {y}");
    }

    #[test]
    fn missing_resolution_is_none() {
        // A non-ResolutionInfo block (id 0x0400) followed by nothing.
        let mut res = Vec::new();
        res.extend_from_slice(b"8BIM");
        res.extend_from_slice(&0x0400u16.to_be_bytes());
        res.extend_from_slice(&[0, 0]); // empty name
        res.extend_from_slice(&0u32.to_be_bytes()); // size 0
        assert!(read_resolution(&psd_prefix(&res)).is_none());
    }

    #[test]
    fn truncated_inputs_return_none_without_panic() {
        assert!(read_resolution(&[]).is_none());
        assert!(read_resolution(&[0u8; HEADER_LEN]).is_none());
        // Resources length claims more bytes than present.
        let mut b = vec![0u8; HEADER_LEN];
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&999u32.to_be_bytes());
        assert!(read_resolution(&b).is_none());
    }

    #[test]
    fn skips_other_blocks_to_find_resolution() {
        let mut res = Vec::new();
        // A 3-byte (odd → padded) leading block of some other id.
        res.extend_from_slice(b"8BIM");
        res.extend_from_slice(&0x0408u16.to_be_bytes());
        res.extend_from_slice(&[0, 0]);
        res.extend_from_slice(&3u32.to_be_bytes());
        res.extend_from_slice(&[1, 2, 3, 0]); // 3 bytes + 1 pad
        res.extend_from_slice(&resolution_resource(72.0, 72.0));
        let (x, y) = read_resolution(&psd_prefix(&res)).expect("resolution present");
        assert!((x - 72.0).abs() < 1e-3 && (y - 72.0).abs() < 1e-3);
    }
}
