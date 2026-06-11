// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel compositor: composites all visible [`LayerTree`] layers into a single
//! wgpu texture representing the current viewport.
//!
//! Phase 2: Normal blend mode only. All other blend modes log a warning and
//! the layer is skipped (Phase 4 work — see SPEC.md §4.8).
//!
//! Sub-modules:
//! - [`pass`] / [`upload`] / [`composite`] — GPU path ([`Compositor::composite`])
//! - [`cpu`] / [`cpu_upload`] — CPU path ([`Compositor::composite_to_texture`]),
//!   safe to call from `CustomPaintSource::render()`

pub(crate) mod pass;
pub(crate) mod upload;
mod composite;
mod cpu;
#[cfg(test)]
mod cpu_tests;
mod cpu_upload;

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
    /// Uses `queue.write_texture()` rather than a `CommandEncoder`, so it cannot
    /// corrupt Vello's in-progress encoder. Inverse-maps destination pixels to
    /// document space, so coverage is complete at any DPI scale, zoom, or
    /// rotation — see [`cpu`] module docs.
    /// TODO(iris): Phase 4 — replace with GPU compute path via render_to_texture().
    pub fn composite_to_texture(
        &self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        // COMPAT(blitz): blitz-paint passes physical pixel dimensions; scale is the
        // DPI factor. Viewport transforms must use logical (CSS) dimensions so they
        // match screen_to_doc() in the event handler. Pixel placement uses physical.
        scale: f64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, CompositorError> {
        cpu_upload::run(tree, viewport, width_px, height_px, scale, device, queue)
    }
}

impl Default for Compositor {
    fn default() -> Self { Self::new() }
}
