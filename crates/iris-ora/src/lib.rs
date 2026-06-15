// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! OpenRaster (`.ora`) import/export adapter.
//!
//! [`OraReader`] converts an OpenRaster document into Iris's native
//! [`iris_aif::AifDocument`] model; [`OraWriter`] serialises one back. Pixel
//! layers and nested groups round-trip, including per-layer position, opacity,
//! visibility, and blend mode (`composite-op`). ORA maps cleanly onto the Iris
//! raster model — there is no vector content in OpenRaster.
//!
//! ```ignore
//! use iris_ora::{OraReader, OraWriter};
//! let doc = OraReader::read(std::path::Path::new("art.ora"))?;
//! OraWriter::write(&doc, std::path::Path::new("copy.ora"))?;
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod composite_op;
mod error;
mod reader;
mod stack;
mod writer;

pub use error::OraError;
pub use reader::OraReader;
pub use writer::OraWriter;
