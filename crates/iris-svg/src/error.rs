// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Typed error enum for the SVG adapter.

/// Errors produced while reading or writing SVG.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SvgError {
    /// The document is not valid XML / SVG.
    #[error("SVG parse error: {0}")]
    Xml(String),

    /// The root `<svg>` element is missing width/height and a usable viewBox.
    #[error("SVG is missing canvas dimensions (width/height or viewBox)")]
    MissingDimensions,

    /// A raster part (embedded `<image>`) failed to decode or encode.
    #[error("raster error: {0}")]
    Aif(#[from] iris_aif::AifError),

    /// I/O error reading or writing the file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_render() {
        assert!(SvgError::MissingDimensions.to_string().contains("dimensions"));
        assert!(SvgError::Xml("bad".into()).to_string().contains("bad"));
    }
}
