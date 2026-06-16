// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`PsdReader`] — the public entry point for importing PSD files.

use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use iris_aif::AifDocument;
use psd::Psd;

use crate::convert::psd_to_document;
use crate::error::PsdError;

/// PSD file length of the fixed file header (§ Photoshop file format).
const PSD_HEADER_LEN: usize = 26;
/// PSD format version (`.psd`); version `2` is PSB and is not yet supported.
const PSD_VERSION: u16 = 1;

/// Stateless reader that imports a Photoshop PSD document into Iris's native
/// [`AifDocument`] model.
pub struct PsdReader;

impl PsdReader {
    /// Read and convert a PSD file from disk.
    pub fn read(path: &Path) -> Result<AifDocument, PsdError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Read and convert a PSD document from any byte source.
    pub fn from_reader<R: Read>(mut reader: R) -> Result<AifDocument, PsdError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes)
    }

    /// Read and convert a PSD document already held in memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<AifDocument, PsdError> {
        validate_header(bytes)?;
        let psd = parse(bytes)?;
        psd_to_document(&psd)
    }
}

/// Validate the 26-byte PSD file header before handing bytes to the parser.
fn validate_header(bytes: &[u8]) -> Result<(), PsdError> {
    if bytes.len() < PSD_HEADER_LEN {
        return Err(PsdError::Truncated(bytes.len()));
    }
    if &bytes[0..4] != b"8BPS" {
        return Err(PsdError::BadSignature);
    }
    // COMPAT(adobe): bytes 4..6 are a big-endian version; 1 = PSD, 2 = PSB.
    let version = u16::from_be_bytes([bytes[4], bytes[5]]);
    if version != PSD_VERSION {
        return Err(PsdError::UnsupportedVersion(version));
    }
    Ok(())
}

/// Parse PSD bytes, converting both parser errors and internal panics from the
/// `psd` crate into a typed [`PsdError::Corrupt`].
fn parse(bytes: &[u8]) -> Result<Psd, PsdError> {
    // COMPAT(adobe): the `psd` crate can panic on inputs it does not fully
    // handle. Catch it so malformed files surface as a recoverable error.
    match catch_unwind(AssertUnwindSafe(|| Psd::from_bytes(bytes))) {
        Ok(Ok(psd)) => Ok(psd),
        Ok(Err(e)) => Err(PsdError::Corrupt(e.to_string())),
        Err(_) => Err(PsdError::Corrupt("PSD parser panicked on malformed input".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_signature() {
        let bytes = vec![0u8; 64];
        assert!(matches!(PsdReader::from_bytes(&bytes), Err(PsdError::BadSignature)));
    }

    #[test]
    fn rejects_truncated() {
        let bytes = b"8BPS".to_vec();
        assert!(matches!(PsdReader::from_bytes(&bytes), Err(PsdError::Truncated(4))));
    }

    #[test]
    fn rejects_psb_version() {
        let mut bytes = vec![0u8; PSD_HEADER_LEN];
        bytes[0..4].copy_from_slice(b"8BPS");
        bytes[4..6].copy_from_slice(&2u16.to_be_bytes());
        assert!(matches!(
            PsdReader::from_bytes(&bytes),
            Err(PsdError::UnsupportedVersion(2))
        ));
    }
}
