// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel compositor: composites all visible [`LayerTree`] layers into a single
//! wgpu texture representing the current viewport.
//!
//! Two paths exist:
//! - [`Compositor::composite`] — GPU blend pass. Not currently driven by the
//!   paint source; see `pass.rs`.
//! - [`Compositor::composite_to_texture`] — the live CPU path, implemented in
//!   [`cpu`]. It avoids creating a `CommandEncoder`, which would corrupt
//!   Vello's in-progress encoder when called from `CustomPaintSource::render()`.
//!
//! Phase 2: Normal blend mode only. Other blend modes are composited as Normal
//! by the CPU path; the GPU path skips them. See TODOs in `composite.rs`.

mod composite;
pub(crate) mod cpu;
pub(crate) mod encode;
pub(crate) mod pass;
pub(crate) mod upload;

use std::sync::{Arc, Mutex};

use iris_pixel::LayerTree;

use crate::viewport::CanvasViewport;

use pass::BlendPass;

/// Errors produced by the compositor.
#[derive(Debug, thiserror::Error)]
pub enum CompositorError {
    /// A wgpu-level operation failed.
    #[error("wgpu compositor error: {0}")]
    Wgpu(String),
}

/// GPU pixel compositor, lazily initialised on first [`Compositor::composite`] call.
///
/// The [`BlendPass`] pipeline is created once and reused across frames.
/// `Arc<Mutex<...>>` allows the compositor to be shared between the Dioxus
/// component (which owns it) and the [`PageSource`] impl (Q5 from audit).
pub struct Compositor {
    state: Arc<Mutex<Option<BlendPass>>>,
}

impl Compositor {
    /// Create a new compositor. Pipeline is initialised lazily.
    pub fn new() -> Self {
        Self { state: Arc::new(Mutex::new(None)) }
    }

    /// Composite all visible pixel layers in `tree` and return the output texture.
    ///
    /// `device` and `queue` arrive from `PageSource::render()`. The pipeline is
    /// initialised on the first call and reused thereafter.
    pub fn composite(
        &self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<appthere_canvas::GpuTexture, CompositorError> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let blend = guard.get_or_insert_with(|| BlendPass::new(device));
        composite::run(blend, tree, viewport, width_px, height_px, device, queue)
    }

    /// CPU composite path — safe to call from `CustomPaintSource::render()`.
    ///
    /// See [`cpu::composite_to_texture`] for the implementation and the
    /// coordinate-space contract around `scale`.
    #[allow(clippy::too_many_arguments)] // mirrors the CustomPaintSource::render signature
    pub fn composite_to_texture(
        &self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        scale: f64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, CompositorError> {
        cpu::composite_to_texture(tree, viewport, width_px, height_px, scale, device, queue)
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}
