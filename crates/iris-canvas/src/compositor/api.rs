// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Public compositor API: [`Compositor`] and [`CompositorError`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use iris_pixel::LayerTree;

use crate::viewport::CanvasViewport;

use super::gpu_frame::GpuFrame;
use super::pass::BlendPass;
use super::{composite, cpu_upload};

/// Errors produced by the compositor.
#[derive(Debug, thiserror::Error)]
pub enum CompositorError {
    /// A wgpu-level operation failed.
    #[error("wgpu compositor error: {0}")]
    Wgpu(String),
}

/// Pixel compositor with lazily-initialised GPU pipelines.
///
/// `Arc<Mutex<...>>` allows the compositor to be shared between the Dioxus
/// component (which owns it) and the [`PageSource`] impl (Q5 from audit).
pub struct Compositor {
    state: Arc<Mutex<Option<BlendPass>>>,
    frame: Arc<Mutex<Option<GpuFrame>>>,
    /// Set after a GPU frame failure; falls back to the CPU path permanently.
    gpu_failed: AtomicBool,
    /// `IRIS_CPU_COMPOSITE=1` forces the CPU frame path (escape hatch).
    gpu_disabled_by_env: bool,
}

impl Compositor {
    /// Create a new compositor. Pipelines are initialised lazily.
    pub fn new() -> Self {
        let gpu_disabled_by_env = std::env::var("IRIS_CPU_COMPOSITE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if gpu_disabled_by_env {
            tracing::info!("IRIS_CPU_COMPOSITE set — GPU frame compositor disabled");
        }
        Self {
            state: Arc::new(Mutex::new(None)),
            frame: Arc::new(Mutex::new(None)),
            gpu_failed: AtomicBool::new(false),
            gpu_disabled_by_env,
        }
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

    /// Frame composite for `CustomPaintSource::render()` — GPU path (Phase 4)
    /// with automatic fallback to the CPU path on error.
    ///
    /// Returns a straight-alpha sRGB `Rgba8Unorm` texture of
    /// `width_px × height_px` physical pixels. `scale` is the DPI factor;
    /// viewport transforms use logical dimensions so they match
    /// `screen_to_doc()` in the event handler.
    pub fn composite_frame(
        &self,
        tree: &LayerTree,
        viewport: &CanvasViewport,
        width_px: u32,
        height_px: u32,
        scale: f64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, CompositorError> {
        if self.gpu_disabled_by_env || self.gpu_failed.load(Ordering::Relaxed) {
            return cpu_upload::run(tree, viewport, width_px, height_px, scale, device, queue);
        }
        let mut guard = self.frame.lock().unwrap_or_else(|e| e.into_inner());
        let frame = guard.get_or_insert_with(|| GpuFrame::new(device, queue));
        match frame.composite(tree, viewport, width_px, height_px, scale, device, queue) {
            Ok(texture) => Ok(texture),
            Err(e) => {
                tracing::error!(error = %e, "GPU frame compositor failed; using CPU path");
                self.gpu_failed.store(true, Ordering::Relaxed);
                cpu_upload::run(tree, viewport, width_px, height_px, scale, device, queue)
            }
        }
    }

    /// Drop per-device GPU state (cached pipelines and tile textures).
    ///
    /// Must be called when the device is suspended or replaced — cached
    /// textures from the previous device must not be used with a new one.
    pub fn reset_device_state(&self) {
        *self.frame.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// CPU composite path — kept for tests and as the `composite_frame`
    /// fallback. Output is identical in format to the GPU frame path.
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
        cpu_upload::run(tree, viewport, width_px, height_px, scale, device, queue)
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

/// Composite to a straight-alpha sRGB RGBA8 byte buffer on the CPU.
///
/// Public so integration tests (and headless thumbnail generation) can obtain
/// a reference image without a GPU device.
pub fn composite_rgba8_cpu(
    tree: &LayerTree,
    viewport: &CanvasViewport,
    width_px: u32,
    height_px: u32,
    scale: f64,
) -> Vec<u8> {
    super::cpu::composite_rgba8(tree, viewport, width_px, height_px, scale)
}
