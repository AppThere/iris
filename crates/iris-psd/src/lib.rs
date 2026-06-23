// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Photoshop PSD import/export adapter.
//!
//! [`PsdReader`] converts a Photoshop `.psd` document into Iris's native
//! [`iris_aif::AifDocument`] model; [`PsdWriter`] serialises one back to PSD
//! (8-bit RGBA pixel layers, group hierarchy, and a flattened merged
//! composite). RGB/Grayscale colour modes are supported. Adjustment layers,
//! masks, smart objects, layer effects, CMYK/Lab colour modes, and PSB (large
//! documents) are deferred to later phases (see `crates/iris-psd/BRIEF.md`).
//!
//! ```ignore
//! use iris_psd::{PsdReader, PsdWriter};
//! let doc = PsdReader::read(std::path::Path::new("art.psd"))?;
//! PsdWriter::write(std::path::Path::new("copy.psd"), &doc)?;
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod blend;
mod convert;
mod error;
mod layers;
mod reader;
mod resources;
mod writer;

pub use error::PsdError;
pub use reader::PsdReader;
pub use writer::PsdWriter;
