// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel compositor: composites all visible [`iris_pixel::LayerTree`] layers
//! into a single wgpu texture representing the current viewport.
//!
//! Phase 2 blend support: Normal only — other blend modes log a warning and
//! the layer is skipped (SPEC.md §4.8, full set arrives with Phase 4 shaders).
//!
//! Sub-modules:
//! - [`api`] — public [`Compositor`] / [`CompositorError`] surface
//! - [`pass`] / [`upload`] / [`composite`] — shared GPU blend pass
//! - [`gpu_frame`] / [`present`] — Phase 4 GPU frame path
//!   ([`Compositor::composite_frame`]) used by the Dioxus paint bridge
//! - [`cpu`] / [`cpu_upload`] — CPU fallback path, also the unit-testable
//!   reference implementation

mod api;
mod composite;
mod cpu;
#[cfg(test)]
mod cpu_tests;
mod cpu_upload;
mod gpu_frame;
pub(crate) mod pass;
mod present;
pub(crate) mod upload;

pub use api::{composite_rgba8_cpu, Compositor, CompositorError};
