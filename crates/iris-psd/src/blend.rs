// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Mapping from the `psd` crate's blend modes to Iris [`BlendMode`]s.

use iris_pixel::BlendMode;

/// Translate a Photoshop blend mode into the Iris equivalent.
///
/// The `psd` crate's `BlendMode` enum lives in a private module and cannot be
/// named by downstream code, so the variant is identified by its `Debug` name
/// (e.g. `"Multiply"`). Callers pass `&format!("{:?}", layer.blend_mode())`.
///
/// Photoshop's `PassThrough` applies only to group layers; a flat pixel layer
/// should never carry it. If one does (malformed file), or the name is
/// unrecognised, it degrades to [`BlendMode::Normal`] with a warning rather
/// than failing the import.
pub(crate) fn map_blend_mode(debug_name: &str) -> BlendMode {
    match debug_name {
        "Normal" => BlendMode::Normal,
        "Dissolve" => BlendMode::Dissolve,
        "Darken" => BlendMode::Darken,
        "Multiply" => BlendMode::Multiply,
        "ColorBurn" => BlendMode::ColorBurn,
        "LinearBurn" => BlendMode::LinearBurn,
        "DarkerColor" => BlendMode::DarkerColor,
        "Lighten" => BlendMode::Lighten,
        "Screen" => BlendMode::Screen,
        "ColorDodge" => BlendMode::ColorDodge,
        "LinearDodge" => BlendMode::LinearDodge,
        "LighterColor" => BlendMode::LighterColor,
        "Overlay" => BlendMode::Overlay,
        "SoftLight" => BlendMode::SoftLight,
        "HardLight" => BlendMode::HardLight,
        "VividLight" => BlendMode::VividLight,
        "LinearLight" => BlendMode::LinearLight,
        "PinLight" => BlendMode::PinLight,
        "HardMix" => BlendMode::HardMix,
        "Difference" => BlendMode::Difference,
        "Exclusion" => BlendMode::Exclusion,
        "Subtract" => BlendMode::Subtract,
        "Divide" => BlendMode::Divide,
        "Hue" => BlendMode::Hue,
        "Saturation" => BlendMode::Saturation,
        "Color" => BlendMode::Color,
        "Luminosity" => BlendMode::Luminosity,
        // COMPAT(adobe): PassThrough is a group-only mode; flat pixel layers
        // must not use it. Degrade gracefully instead of erroring.
        other => {
            tracing::warn!(blend_mode = other, "unhandled PSD blend mode; treating as Normal");
            BlendMode::Normal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_modes() {
        assert_eq!(map_blend_mode("Normal"), BlendMode::Normal);
        assert_eq!(map_blend_mode("Multiply"), BlendMode::Multiply);
        assert_eq!(map_blend_mode("Luminosity"), BlendMode::Luminosity);
    }

    #[test]
    fn pass_through_and_unknown_degrade_to_normal() {
        assert_eq!(map_blend_mode("PassThrough"), BlendMode::Normal);
        assert_eq!(map_blend_mode("☃"), BlendMode::Normal);
    }
}
