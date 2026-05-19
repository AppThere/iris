// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`IrisCanvas`] — Dioxus component that composites a [`LayerTree`] into a
//! wgpu-rendered canvas element via Dioxus Native's `use_wgpu` hook.
//!
//! # Architecture
//!
//! Dioxus Native (Blitz) owns the GPU device and calls the paint source on
//! every frame. The bridge:
//!   1. `IrisCanvas` holds shared `Arc<Mutex<…>>` state for the viewport and
//!      layer tree, updated on each Dioxus re-render.
//!   2. `use_wgpu` registers an [`IrisCanvasPaintSource`] with the Blitz
//!      renderer; it calls `Compositor::composite()` each frame, hands the
//!      resulting `wgpu::Texture` to `CustomPaintCtx::register_texture`, and
//!      returns the `TextureHandle` for `<canvas src="…">`.
//!   3. `IrisPageSource` implements `appthere_canvas::PageSource` for
//!      non-Dioxus contexts (e.g. headless rendering, tests).
//!
//! Hooks rule: every `use_*` call is unconditional at the top level of the
//! component body — no hooks inside closures, conditions, or loops.

use std::sync::{Arc, Mutex};

use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use iris_pixel::LayerTree;

use crate::compositor::Compositor;
use crate::paint_bridge::IrisCanvasPaintSource;
use crate::viewport::CanvasViewport;

/// Props for the [`IrisCanvas`] component.
#[derive(Props, Clone, PartialEq)]
pub struct IrisCanvasProps {
    /// Reactive document layer tree. Component re-renders when the signal changes.
    pub tree: Signal<LayerTree>,
    /// Reactive viewport state (pan, zoom, rotation). Component re-renders on change.
    pub viewport: Signal<CanvasViewport>,
    /// Canvas width in CSS pixels.
    pub width: u32,
    /// Canvas height in CSS pixels.
    pub height: u32,
}

/// Dioxus component for the Iris infinite canvas.
///
/// Renders via Dioxus Native's `use_wgpu` + Blitz `<canvas src="{id}">`.
/// Reactive signals keep viewport and layer tree in sync with the component tree.
#[component]
pub fn IrisCanvas(props: IrisCanvasProps) -> Element {
    // Lazily-initialised compositor, shared with the paint source.
    let compositor = use_hook(|| Arc::new(Mutex::new(Compositor::new())));

    // Shared viewport and tree: initialised once, updated each re-render so the
    // paint source always sees the latest state without needing Dioxus context.
    let shared_viewport = use_hook(|| Arc::new(Mutex::new(props.viewport.peek().clone())));
    let shared_tree = use_hook(|| Arc::new(Mutex::new(props.tree.peek().clone())));

    // Sync signal values into shared state on every re-render.
    if let Ok(mut vp) = shared_viewport.try_lock() {
        *vp = props.viewport.read().clone();
    }
    if let Ok(mut tr) = shared_tree.try_lock() {
        *tr = props.tree.read().clone();
    }

    // Register the paint source with Blitz. `use_wgpu` uses `use_hook_with_cleanup`
    // internally — auto-unregisters when the component is dropped. FnOnce captures
    // cloned Arcs, so the shared state remains live for the component's lifetime.
    let canvas_id = use_wgpu(|| {
        IrisCanvasPaintSource::new(
            compositor.clone(),
            shared_viewport.clone(),
            shared_tree.clone(),
        )
    });

    rsx! {
        canvas {
            // "src" is not in Dioxus's canvas element schema but blitz-dom reads
            // it to associate a registered CustomPaintSource with this element.
            "src": "{canvas_id}",
            width: "{props.width}",
            height: "{props.height}",
            style: "display: block; width: {props.width}px; height: {props.height}px;",
        }
    }
}

/// [`appthere_canvas::PageSource`] implementation for the Iris compositor.
///
/// The unit key `()` represents the single composited viewport. Useful for
/// headless rendering and non-Dioxus contexts. Dioxus Native rendering uses
/// [`IrisCanvasPaintSource`] via `use_wgpu` instead.
pub struct IrisPageSource {
    compositor: Arc<Mutex<Compositor>>,
    width: u32,
    height: u32,
    viewport: CanvasViewport,
    tree: LayerTree,
}

impl IrisPageSource {
    /// Create a new page source from current component state.
    pub fn new(
        compositor: Arc<Mutex<Compositor>>,
        width: u32,
        height: u32,
        viewport: CanvasViewport,
        tree: LayerTree,
    ) -> Self {
        Self { compositor, width, height, viewport, tree }
    }
}

impl appthere_canvas::PageSource for IrisPageSource {
    type Key = ();

    fn page_size_px(&self, _index: ()) -> (u32, u32) {
        (self.width, self.height)
    }

    fn render(
        &self,
        _index: (),
        _scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<appthere_canvas::GpuTexture, appthere_canvas::RenderError> {
        let guard = self.compositor.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .composite(&self.tree, &self.viewport, self.width, self.height, device, queue)
            .map_err(|e| appthere_canvas::RenderError::Wgpu(e.to_string()))
    }
}
