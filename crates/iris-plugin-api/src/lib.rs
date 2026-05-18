// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Plugin API surface for AppThere Iris.
//!
//! See SPEC.md §9 and ADR/005-plugin-wasi.md before implementing.
//!
//! Phase 1: trait boundary only. No wasmtime dependency.
//! Phase 5: full WASI sandbox implementation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Marker trait for all Iris plugins.
///
/// Full implementation is Phase 5. See ADR/005-plugin-wasi.md.
pub trait IrisPlugin: Send + Sync {
    /// Human-readable plugin name.
    fn name(&self) -> &str;
    /// Semver version string.
    fn version(&self) -> &str;
}

/// Plugin loader — desktop and Android only.
///
// COMPAT(mobile): ADR-005 — user-loadable plugins excluded from iOS builds.
// On iOS, this module compiles to an empty stub. All plugin-loading code
// added in Phase 5 must live inside the cfg(not(target_os = "ios")) gate.
#[cfg(not(target_os = "ios"))]
pub mod loader {
    // TODO(iris): SPEC.md §9 — Phase 5: wasmtime Cranelift loader for
    // desktop (Windows, macOS, Linux) and Android.
}

/// Plugin loader stub — iOS only.
///
// COMPAT(mobile): ADR-005 — see above.
#[cfg(target_os = "ios")]
pub mod loader {
    // TODO(iris): SPEC.md §9 — Phase 5 Option A: wasmtime Pulley loader.
    // Blocked on Pulley iOS track record. See ADR 005 for transition criteria.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iris_plugin_is_object_safe() {
        // Confirm the trait can be used as a trait object.
        let _: Option<Box<dyn IrisPlugin>> = None;
    }
}
