// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Bridge between [`Compositor`] and `anyrender_vello::CustomPaintSource`.
//!
//! API confirmed from anyrender_vello 0.6.2 / wgpu_context 0.1.2:
//!   - `DeviceHandle { pub device: wgpu::Device, pub queue: wgpu::Queue, … }`
//!     All fields are public; `DeviceHandle: Clone` (wgpu::Device/Queue are
//!     reference-counted handles and are also Clone).
//!   - `CustomPaintCtx::register_texture(wgpu::Texture) -> TextureHandle`
//!     Takes `Texture` by value. `anyrender_vello` depends on `wgpu = "26"`,
//!     the same version iris-canvas uses — types are compatible.
//!   - `CustomPaintCtx::unregister_texture(TextureHandle)` — takes by value.
//!   - `TextureHandle: Clone` — safe to clone when both storing and returning.
//!   - `CustomPaintSource::render()` returning `None` skips the frame;
//!     Blitz reuses the last registered texture until a `Some` is returned.

use std::sync::{Arc, Mutex};

use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use iris_pixel::LayerTree;

use crate::compositor::Compositor;
use crate::viewport::CanvasViewport;

/// Bridges [`Compositor`] into `anyrender_vello::CustomPaintSource` so that
/// Dioxus Native's `use_wgpu` hook can drive the Iris render pipeline.
///
/// `viewport` and `tree` are `Arc<Mutex<…>>` shared with the `IrisCanvas`
/// component body, which updates them on every re-render. The Blitz paint
/// loop calls `render()` independently; no Dioxus reactive context is needed.
pub(crate) struct IrisCanvasPaintSource {
    compositor: Arc<Mutex<Compositor>>,
    viewport: Arc<Mutex<CanvasViewport>>,
    tree: Arc<Mutex<LayerTree>>,
    device_handle: Option<DeviceHandle>,
    last_handle: Option<TextureHandle>,
}

impl IrisCanvasPaintSource {
    pub(crate) fn new(
        compositor: Arc<Mutex<Compositor>>,
        viewport: Arc<Mutex<CanvasViewport>>,
        tree: Arc<Mutex<LayerTree>>,
    ) -> Self {
        Self { compositor, viewport, tree, device_handle: None, last_handle: None }
    }
}

impl CustomPaintSource for IrisCanvasPaintSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.device_handle = Some(device_handle.clone());
    }

    fn suspend(&mut self) {
        self.device_handle = None;
        self.last_handle = None;
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        let dh = self.device_handle.as_ref()?;

        if let Some(old) = self.last_handle.take() {
            ctx.unregister_texture(old);
        }

        // Clone viewport (CanvasViewport: Copy+Clone); hold tree and compositor
        // guards only for the duration of the composite call.
        let viewport = self.viewport.lock().ok()?.clone();
        let tree_guard = self.tree.lock().ok()?;
        let compositor_guard = self.compositor.lock().ok()?;

        // CPU composite path: uses queue.write_texture() rather than a CommandEncoder
        // submission. Submitting a CommandEncoder here corrupts Vello's in-progress encoder,
        // causing the "Encoder is invalid" crash.
        // TODO(iris): Phase 4 — replace with GPU compute path that follows Loki's
        // render_to_texture() pattern so work is submitted through Vello's encoder.
        let texture = compositor_guard
            .composite_to_texture(&tree_guard, &viewport, width, height, &dh.device, &dh.queue)
            .ok()?;

        drop(compositor_guard);
        drop(tree_guard);

        // scale is passed through for future use; current compositor uses width/height directly.
        let _ = scale;

        let handle = ctx.register_texture(texture);
        self.last_handle = Some(handle.clone());
        Some(handle)
    }
}
