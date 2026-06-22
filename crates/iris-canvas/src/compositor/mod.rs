// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel compositor: composites all visible [`iris_pixel::LayerTree`] layers
//! into a single wgpu texture representing the current viewport.
//!
//! Blend support: the CPU reference path ([`cpu`]) composites all 27 blend
//! modes via [`iris_pixel::blend`]. The GPU paths ([`composite`], [`gpu_frame`])
//! still wire only Normal — other modes log a warning and the layer is skipped
//! until the backdrop-sampling blend shader lands (SPEC.md §4.8).
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
mod cpu_color;
#[cfg(test)]
mod cpu_tests;
mod cpu_upload;
mod gpu_frame;
pub(crate) mod pass;
mod present;
pub(crate) mod upload;

pub use api::{composite_rgba8_cpu, Compositor, CompositorError};
