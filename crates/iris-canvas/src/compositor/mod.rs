// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Pixel compositor: composites all visible [`LayerTree`] layers into a single
//! `Rgba16Float` wgpu texture representing the current viewport.
//!
//! Phase 2: Normal blend mode only. All other blend modes log a warning and
//! the layer is skipped. See TODO below for Phase 4.

pub(crate) mod pass;
pub(crate) mod upload;
mod composite;

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
}

impl Default for Compositor {
    fn default() -> Self { Self::new() }
}
