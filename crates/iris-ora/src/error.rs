// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Typed error enum for the OpenRaster adapter.

/// Errors produced while reading or writing an OpenRaster (`.ora`) file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OraError {
    /// The archive's `mimetype` entry is missing or not `image/openraster`.
    #[error("not an OpenRaster file (missing or wrong mimetype)")]
    BadMimetype,

    /// The archive does not contain the required `stack.xml`.
    #[error("missing stack.xml")]
    MissingStackXml,

    /// `stack.xml` is missing the `<image>` width/height attributes.
    #[error("stack.xml: missing or invalid image dimensions")]
    MissingImageDimensions,

    /// `stack.xml` could not be parsed.
    #[error("stack.xml parse error: {0}")]
    Xml(String),

    /// The ZIP container could not be read or written.
    #[error("ORA container error: {0}")]
    Zip(String),

    /// A layer references a `src` part that is not present in the archive.
    #[error("missing layer data part '{0}'")]
    MissingLayerData(String),

    /// A raster part failed to decode or encode.
    #[error("raster error: {0}")]
    Aif(#[from] iris_aif::AifError),

    /// I/O error reading or writing the file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<zip::result::ZipError> for OraError {
    fn from(e: zip::result::ZipError) -> Self {
        OraError::Zip(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_render() {
        assert!(OraError::BadMimetype.to_string().contains("OpenRaster"));
        assert!(OraError::MissingLayerData("data/3.png".into())
            .to_string()
            .contains("data/3.png"));
    }
}
