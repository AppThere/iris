// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Vector layer path store — `iris/layers/{id}/paths.bin` (SPEC.md §4.10).
//!
//! A vector layer has no tile directory; its geometry lives in a single
//! FlatBuffers part decoded to / encoded from [`iris_vector::VectorLayer`].
//!
//! See [`wire`] for why decoding is hand-rolled rather than delegated to
//! `flatc`-generated accessors.

mod decode;
mod encode;
mod schema;
mod wire;

pub(crate) use decode::read_path_store;
pub(crate) use encode::write_path_store;

#[cfg(test)]
mod tests;
