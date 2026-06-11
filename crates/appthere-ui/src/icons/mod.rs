// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`AtIcon`] — renders a vendored [Lucide](https://lucide.dev) icon as an
//! inline SVG element.
//!
//! Icon markup constants live in [`lucide`]; pass one to the `icon` prop:
//!
//! ```rust,ignore
//! AtIcon { icon: icons::lucide::BRUSH, size: 18.0, color: "#e8e8e8".to_string() }
//! ```
//!
//! // COMPAT(dioxus-native): Blitz parses inline `<svg>` elements through
//! // usvg by serialising the element's outer HTML. `currentColor` resolution
//! // is unreliable in that path, so the stroke colour is set as a concrete
//! // value on the `<svg>` element rather than inherited from CSS `color`.

pub mod lucide;

use dioxus::prelude::*;

/// Default icon edge length in logical pixels.
pub const ICON_SIZE_DEFAULT: f32 = 18.0;

/// Inline SVG icon. `icon` is one of the [`lucide`] markup constants.
///
/// Lucide icons are stroke-based on a 24×24 viewBox with round caps/joins;
/// the wrapper element supplies those presentation attributes so the vendored
/// constants stay minimal.
#[component]
pub fn AtIcon(
    /// Inner SVG markup — one of the [`lucide`] constants.
    icon: &'static str,
    /// Rendered edge length in logical pixels.
    #[props(default = ICON_SIZE_DEFAULT)]
    size: f32,
    /// Concrete stroke colour (e.g. a `tokens::colors` value).
    color: String,
) -> Element {
    rsx! {
        svg {
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "{color}",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            dangerous_inner_html: "{icon}",
        }
    }
}
