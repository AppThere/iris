# Vendored Crate Patches

This document describes every patched dependency in `crates/patches/`.
Each entry explains what was wrong, what was changed, and when to remove it.

---

## blitz-dom (v0.2.4)

**File:** `crates/patches/blitz-dom/`
**Upstream:** <https://crates.io/crates/blitz-dom>

**Problem:** `tabindex` focus-on-click did not work for non-`<input>` elements,
so the wgpu canvas element could not receive keyboard focus after a click.

**Fix:** Override `handle_click` to set focus on any focusable element.

**Remove when:** upstream blitz-dom makes tabindex focus on click work for
non-input elements.

---

## blitz-shell (v0.2.3)

**File:** `crates/patches/blitz-shell/`
**Upstream:** <https://crates.io/crates/blitz-shell>

**Problem:** `WindowEvent::Touch` was silently discarded — Dioxus
`ontouchstart` / `ontouchmove` / `ontouchend` handlers never fired on
touch-screen devices.

**Fix:** Synthesise `UiEvent::MouseDown` / `MouseMove` / `MouseUp` events
from touch contacts so that Dioxus touch handlers receive the correct
coordinates. Only single-contact touch is forwarded; multi-touch requires
native `UiEvent::Touch*` variants which do not exist in blitz-traits 0.2.x.

**Remove when:** blitz-shell implements native touch event routing.

---

## dioxus-native-dom (v0.7.9)

**File:** `crates/patches/dioxus-native-dom/`
**Upstream:** <https://crates.io/crates/dioxus-native-dom>

**Problem:** `HtmlEventConverter` methods for touch, pointer, and wheel
events were all `unimplemented!()`, causing runtime panics when Dioxus
dispatched those event types.

**Fix:**
- `convert_touch_data` — extracts coordinates from the synthesised
  `NativeClickData` produced by the blitz-shell touch patch.
- `convert_pointer_data` — delegates to `NativeClickData`, synthesising a
  `"mouse"` type primary pointer (pressure 0.5 when buttons held, 0.0
  otherwise).
- `convert_wheel_data` — returns a zero-delta `WheelData` via `NativeWheelData`
  instead of panicking. The event itself is not yet routed by blitz-shell
  (see TODO in `canvas_widget.rs §11.2`); this prevents crashes if routing
  is added in future.

**Remove when:** upstream dioxus-native-dom implements these converters.

---

## wgpu_context (v0.1.2)

**File:** `crates/patches/wgpu-context/`
**Upstream:** <https://crates.io/crates/wgpu_context>

**Problem:** `maybe_blit_and_present()` calls `.expect()` on
`Surface::get_current_texture()`, panicking on `SurfaceError::Outdated`
and `SurfaceError::Lost`.

On Windows/DX12, registering a `CustomPaintSource` via `use_wgpu` triggers
an extra render cycle during window initialisation that returns `Outdated`
before the swap chain is ready. The original author noted this as a known
gap with a TODO comment at `surface_renderer.rs:228`.

**Fix:** Match on the error and return early (skip the frame) for `Outdated`
and `Lost`. The next redraw event retries and succeeds.
`SurfaceError::Timeout` and unknown errors still panic.

**Remove when:** wgpu_context upstream fixes the TODO comment at
`surface_renderer.rs:228` ("verify that handling of SurfaceError::Outdated
is no longer required").
