// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Photoshop PSD import adapter.
//!
//! Converts a Photoshop `.psd` document into Iris's native
//! [`iris_aif::AifDocument`] model. Phase 1 supports flat RGB/Grayscale pixel
//! layers; groups, adjustment layers, masks, smart objects, layer effects,
//! CMYK/Lab colour modes, PSB (large documents), and PSD *writing* are
//! deferred to later phases (see `crates/iris-psd/BRIEF.md`).
//!
//! ```ignore
//! use iris_psd::PsdReader;
//! let doc = PsdReader::read(std::path::Path::new("art.psd"))?;
//! println!("{} layers", doc.layers.root_layer_ids().len());
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod blend;
mod convert;
mod error;
mod layers;
mod reader;

pub use error::PsdError;
pub use reader::PsdReader;
