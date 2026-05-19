// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`IrisCanvas`] — Dioxus component that composites a [`LayerTree`] into a
//! wgpu-rendered canvas element.
//!
//! # BLOCKING — appthere-canvas Dioxus paint hook not yet available
//!
//! `appthere-canvas/src/dioxus/` contains only `scroll_driver.rs` (settle
//! detector). There is no Dioxus hook or component that calls
//! `PageSource::render()` and presents the resulting `GpuTexture` to the
//! Dioxus/Blitz render tree.
//!
//! The `PageSource<Key = ()>` implementation on [`IrisPageSource`] is complete
//! and correct. The blocking gap is the bridge from Dioxus component → wgpu
//! device acquisition → `render()` invocation. Until appthere-canvas ships
//! that bridge (e.g. a `use_canvas_paint` hook), `IrisCanvas` renders an
//! empty placeholder div.
//!
// TODO(iris): SPEC.md §6.2 — BLOCKED: implement use_canvas_paint (or equivalent)
//   in appthere-canvas/src/dioxus/ that calls PageSource::render() and feeds
//   the GpuTexture to Dioxus Native's CustomPaintSource. Then wire IrisCanvas
//   to call it here.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use iris_pixel::LayerTree;

use crate::compositor::Compositor;
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
/// Phase 2 renders an empty placeholder until the appthere-canvas Dioxus paint
/// hook is available (see module-level BLOCKED comment above).
///
/// Hooks rule (lesson from iris-aif): every `use_*` call is at the top level of
/// the component body — never inside a closure, conditional, or loop.
#[component]
pub fn IrisCanvas(props: IrisCanvasProps) -> Element {
    // Lazily-initialised compositor, shared with the PageSource impl.
    // Stored in use_hook so it survives re-renders without reinitialisation.
    let compositor = use_hook(|| Arc::new(Mutex::new(Compositor::new())));

    // TODO(iris): SPEC.md §6.2 — Phase 2+: use_settle_detector for quality-tier
    // promotion. Wire when the appthere-canvas Dioxus paint hook is available.
    // let (task, _tx) = use_settle_detector(scroll_signal, || compositor.mark_all_dirty());
    // use_drop(move || task.cancel());

    // TODO(iris): SPEC.md §6.2 — BLOCKED: mount IrisPageSource via
    // appthere_canvas::dioxus::use_canvas_paint (not yet implemented in
    // appthere-canvas). Once available, replace the placeholder div below.
    let _ = compositor; // suppress unused warning until paint hook is wired

    rsx! {
        div {
            style: "width: {props.width}px; height: {props.height}px; background: #1a1a1a;",
            // TODO(iris): SPEC.md §6.2 — BLOCKED: replace placeholder div with
            // wgpu-rendered canvas once appthere-canvas ships a Dioxus paint hook.
        }
    }
}

/// [`appthere_canvas::PageSource`] implementation for the Iris compositor.
///
/// The unit key `()` represents the single composited viewport — there is
/// exactly one "page" per canvas view (Q4 decision from audit).
///
/// The `render()` call triggers the full compositor pass: iterate visible
/// layers, upload tiles, blend via Normal-mode render pipeline, return texture.
pub struct IrisPageSource {
    compositor: Arc<Mutex<Compositor>>,
    width: u32,
    height: u32,
    // The viewport is cloned at render time from the Dioxus signal read.
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
