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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use iris_pixel::LayerTree;

use crate::compositor::Compositor;
use crate::viewport::CanvasViewport;

/// Bridges [`Compositor`] into `anyrender_vello::CustomPaintSource` so that
/// Dioxus Native's `use_wgpu` hook can drive the Iris render pipeline.
///
/// `viewport`, `tree`, and `rendered_size` are `Arc<Mutex<…>>` shared with
/// the `IrisCanvas` component body. The Blitz paint loop calls `render()`
/// independently; no Dioxus reactive context is needed.
///
/// `rendered_size` is written on every `render()` call with the true Blitz
/// layout dimensions, ensuring event handlers always see the correct size
/// regardless of when `onmounted` fires relative to the first click.
pub(crate) struct IrisCanvasPaintSource {
    compositor: Arc<Mutex<Compositor>>,
    viewport: Arc<Mutex<CanvasViewport>>,
    tree: Arc<Mutex<LayerTree>>,
    /// Written each frame by `render()` with the Blitz-reported canvas size.
    rendered_size: Arc<Mutex<(u32, u32)>>,
    /// Bumped by the component whenever tree/viewport state changes; lets
    /// `render()` skip recompositing entirely on unchanged frames.
    generation: Arc<AtomicU64>,
    /// `(generation, width, height, scale bits)` of the last composited frame.
    last_frame: Option<(u64, u32, u32, u64)>,
    device_handle: Option<DeviceHandle>,
    last_handle: Option<TextureHandle>,
}

impl IrisCanvasPaintSource {
    pub(crate) fn new(
        compositor: Arc<Mutex<Compositor>>,
        viewport: Arc<Mutex<CanvasViewport>>,
        tree: Arc<Mutex<LayerTree>>,
        rendered_size: Arc<Mutex<(u32, u32)>>,
        generation: Arc<AtomicU64>,
    ) -> Self {
        Self {
            compositor,
            viewport,
            tree,
            rendered_size,
            generation,
            last_frame: None,
            device_handle: None,
            last_handle: None,
        }
    }
}

impl CustomPaintSource for IrisCanvasPaintSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.device_handle = Some(device_handle.clone());
        // A new device invalidates cached pipelines and tile textures.
        if let Ok(c) = self.compositor.lock() {
            c.reset_device_state();
        }
        self.last_frame = None;
    }

    fn suspend(&mut self) {
        self.device_handle = None;
        self.last_handle = None;
        if let Ok(c) = self.compositor.lock() {
            c.reset_device_state();
        }
        self.last_frame = None;
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        // COMPAT(blitz): blitz-paint passes PHYSICAL pixel dimensions to render()
        // (content_box.width() = layout.size.width * scale, per blitz-paint/render.rs).
        // Event coordinates (element_coordinates()) are in LOGICAL CSS pixels (Winit
        // logical cursor position minus Taffy absolute_position, both in CSS px).
        // Divide by scale to get the logical canvas size that screen_to_doc() expects.
        let logical_w = ((width as f64) / scale.max(1.0)).round().max(1.0) as u32;
        let logical_h = ((height as f64) / scale.max(1.0)).round().max(1.0) as u32;
        // Write logical size first — must succeed even if compositing fails later.
        if let Ok(mut sz) = self.rendered_size.try_lock() {
            *sz = (logical_w, logical_h);
        }
        tracing::debug!(
            physical_w = width,
            physical_h = height,
            scale = scale,
            logical_w = logical_w,
            logical_h = logical_h,
            "IrisCanvasPaintSource::render size"
        );

        let dh = self.device_handle.as_ref()?;

        // Skip recompositing when nothing changed since the last frame: Blitz
        // calls render() every paint, but the canvas only changes when the
        // component bumps `generation` (stroke, pan/zoom, resize). Returning
        // the existing handle reuses the registered texture for free.
        let generation = self.generation.load(Ordering::Acquire);
        let frame_key = (generation, width, height, scale.to_bits());
        if self.last_frame == Some(frame_key) {
            if let Some(handle) = &self.last_handle {
                return Some(handle.clone());
            }
        }

        if let Some(old) = self.last_handle.take() {
            ctx.unregister_texture(old);
        }

        // Clone viewport (CanvasViewport: Copy+Clone); hold tree and compositor
        // guards only for the duration of the composite call.
        let viewport = self.viewport.lock().ok()?.clone();
        let tree_guard = self.tree.lock().ok()?;
        let compositor_guard = self.compositor.lock().ok()?;

        // Phase 4 GPU frame path. render() runs during Vello scene building —
        // before Vello encodes its own pass — so our queue.submit() lands
        // ahead of the pass that samples the registered texture. Falls back
        // to the CPU path on GPU error or when IRIS_CPU_COMPOSITE=1.
        let texture = compositor_guard
            .composite_frame(&tree_guard, &viewport, width, height, scale, &dh.device, &dh.queue)
            .ok()?;

        drop(compositor_guard);
        drop(tree_guard);

        let handle = ctx.register_texture(texture);
        self.last_handle = Some(handle.clone());
        self.last_frame = Some(frame_key);
        Some(handle)
    }
}
