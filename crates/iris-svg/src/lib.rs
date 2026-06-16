// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! SVG 1.1 import/export adapter.
//!
//! [`SvgReader`] converts an SVG document into Iris's native
//! [`iris_aif::AifDocument`] (vector geometry into a `LayerContent::Vector`
//! layer; embedded `<image>` data URIs into raster layers). [`SvgWriter`]
//! serialises one back: vector objects to `<path>` elements and raster layers
//! to base64-encoded `<image>` elements.
//!
//! Phase 1 covers paths and basic shapes, groups, transforms, solid
//! fills/strokes, and embedded raster images. Gradients, text, filters, clip
//! paths, masks, and patterns are deferred (see `crates/iris-svg/BRIEF.md`).
//!
//! ```ignore
//! use iris_svg::{SvgReader, SvgWriter};
//! let doc = SvgReader::read(std::path::Path::new("art.svg"))?;
//! SvgWriter::write(&doc, std::path::Path::new("copy.svg"))?;
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod attrs;
mod base64;
mod color;
mod error;
mod reader;
mod shapes;
mod style;
mod transform;
mod writer;

pub use error::SvgError;
pub use reader::SvgReader;
pub use writer::SvgWriter;
