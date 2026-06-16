// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Mapping between OpenRaster `composite-op` identifiers and Iris [`BlendMode`].

use iris_pixel::BlendMode;

/// Parse an ORA `composite-op` string into an Iris [`BlendMode`].
///
/// Unknown identifiers fall back to [`BlendMode::Normal`] with a warning so an
/// unfamiliar mode never aborts the import.
pub(crate) fn from_ora(op: &str) -> BlendMode {
    match op {
        "svg:src-over" => BlendMode::Normal,
        "svg:multiply" => BlendMode::Multiply,
        "svg:screen" => BlendMode::Screen,
        "svg:overlay" => BlendMode::Overlay,
        "svg:darken" => BlendMode::Darken,
        "svg:lighten" => BlendMode::Lighten,
        "svg:color-dodge" => BlendMode::ColorDodge,
        "svg:color-burn" => BlendMode::ColorBurn,
        "svg:hard-light" => BlendMode::HardLight,
        "svg:soft-light" => BlendMode::SoftLight,
        "svg:difference" => BlendMode::Difference,
        "svg:color" => BlendMode::Color,
        "svg:luminosity" => BlendMode::Luminosity,
        "svg:hue" => BlendMode::Hue,
        "svg:saturation" => BlendMode::Saturation,
        "svg:exclusion" => BlendMode::Exclusion,
        "svg:plus" => BlendMode::LinearDodge,
        // COMPAT(krita): Krita emits vendor-prefixed ops for modes outside the
        // ORA/SVG core set.
        "krita:add" => BlendMode::LinearDodge,
        "krita:subtract" => BlendMode::Subtract,
        "krita:divide" => BlendMode::Divide,
        "krita:linear_burn" => BlendMode::LinearBurn,
        "krita:vivid_light" => BlendMode::VividLight,
        "krita:linear_light" => BlendMode::LinearLight,
        "krita:pin_light" => BlendMode::PinLight,
        "krita:hard_mix" => BlendMode::HardMix,
        "krita:darken_color" | "krita:darker_color" => BlendMode::DarkerColor,
        "krita:lighten_color" | "krita:lighter_color" => BlendMode::LighterColor,
        "krita:dissolve" => BlendMode::Dissolve,
        other => {
            tracing::warn!(composite_op = other, "unhandled ORA composite-op; treating as Normal");
            BlendMode::Normal
        }
    }
}

/// Return the ORA `composite-op` identifier for an Iris [`BlendMode`].
///
/// Modes outside the ORA/SVG core set use Krita's vendor prefix so the value
/// round-trips through [`from_ora`].
pub(crate) fn to_ora(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "svg:src-over",
        BlendMode::Multiply => "svg:multiply",
        BlendMode::Screen => "svg:screen",
        BlendMode::Overlay => "svg:overlay",
        BlendMode::Darken => "svg:darken",
        BlendMode::Lighten => "svg:lighten",
        BlendMode::ColorDodge => "svg:color-dodge",
        BlendMode::ColorBurn => "svg:color-burn",
        BlendMode::HardLight => "svg:hard-light",
        BlendMode::SoftLight => "svg:soft-light",
        BlendMode::Difference => "svg:difference",
        BlendMode::Color => "svg:color",
        BlendMode::Luminosity => "svg:luminosity",
        BlendMode::Hue => "svg:hue",
        BlendMode::Saturation => "svg:saturation",
        BlendMode::Exclusion => "svg:exclusion",
        BlendMode::LinearDodge => "svg:plus",
        BlendMode::Subtract => "krita:subtract",
        BlendMode::Divide => "krita:divide",
        BlendMode::LinearBurn => "krita:linear_burn",
        BlendMode::VividLight => "krita:vivid_light",
        BlendMode::LinearLight => "krita:linear_light",
        BlendMode::PinLight => "krita:pin_light",
        BlendMode::HardMix => "krita:hard_mix",
        BlendMode::DarkerColor => "krita:darker_color",
        BlendMode::LighterColor => "krita:lighter_color",
        BlendMode::Dissolve => "krita:dissolve",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_ops_round_trip() {
        for mode in [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Difference,
            BlendMode::Subtract,
            BlendMode::Divide,
        ] {
            assert_eq!(from_ora(to_ora(mode)), mode, "round-trip failed for {mode:?}");
        }
    }

    #[test]
    fn unknown_op_is_normal() {
        assert_eq!(from_ora("svg:nonsense"), BlendMode::Normal);
    }
}
