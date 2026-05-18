// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`BlendMode`] enum covering all 27 Photoshop-compatible compositing modes
//! from SPEC.md §4.8, plus AIF string round-trip conversion.

/// Compositing blend mode for a layer.
///
/// All 27 variants are present so that AIF files can be loaded and re-saved
/// without loss, even though the Phase 1 compositor only wires `Normal`.
/// The remaining modes are wired in Phase 4 (`iris-canvas`).
// TODO(iris): SPEC.md §4.8 — Phase 4: wire non-Normal variants in iris-canvas WGSL shaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// Standard alpha compositing (Porter-Duff over).
    Normal,
    /// Pixel-level random transparency based on opacity.
    Dissolve,
    /// Keeps the darker of the source and destination pixels.
    Darken,
    /// Multiplies source and destination values.
    Multiply,
    /// Darkens by increasing contrast between layers.
    ColorBurn,
    /// Darkens by decreasing brightness.
    LinearBurn,
    /// Keeps whichever pixel is darker across all channels.
    DarkerColor,
    /// Keeps the lighter of the source and destination pixels.
    Lighten,
    /// Multiplies the inverse of source and destination values.
    Screen,
    /// Lightens by decreasing contrast.
    ColorDodge,
    /// Lightens by increasing brightness.
    LinearDodge,
    /// Keeps whichever pixel is lighter across all channels.
    LighterColor,
    /// Multiplies or screens depending on the destination value.
    Overlay,
    /// Similar to Overlay with a softer result.
    SoftLight,
    /// Similar to Overlay with a harder result.
    HardLight,
    /// Burns or dodges based on the blend colour.
    VividLight,
    /// Burns or dodges by decreasing or increasing brightness.
    LinearLight,
    /// Replaces based on the blend colour compared to 50% grey.
    PinLight,
    /// Posterises by applying a hard-mix of colour channels.
    HardMix,
    /// Subtracts the darker value from the lighter.
    Difference,
    /// Similar to Difference with lower contrast.
    Exclusion,
    /// Subtracts the blend layer from the base.
    Subtract,
    /// Divides the base layer by the blend layer.
    Divide,
    /// Applies the hue of the blend with luminosity and saturation of the base.
    Hue,
    /// Applies the saturation of the blend with hue and luminosity of the base.
    Saturation,
    /// Applies hue and saturation of the blend with luminosity of the base.
    Color,
    /// Applies the luminosity of the blend with hue and saturation of the base.
    Luminosity,
}

impl BlendMode {
    /// Parse a blend mode from its AIF string identifier (SPEC.md §4.8).
    ///
    /// Returns `None` for unknown identifiers; callers should treat unknown
    /// modes as `Normal` with a `tracing::warn!` rather than erroring.
    pub fn from_aif_str(s: &str) -> Option<Self> {
        match s {
            "normal"        => Some(Self::Normal),
            "dissolve"      => Some(Self::Dissolve),
            "darken"        => Some(Self::Darken),
            "multiply"      => Some(Self::Multiply),
            "color-burn"    => Some(Self::ColorBurn),
            "linear-burn"   => Some(Self::LinearBurn),
            "darker-color"  => Some(Self::DarkerColor),
            "lighten"       => Some(Self::Lighten),
            "screen"        => Some(Self::Screen),
            "color-dodge"   => Some(Self::ColorDodge),
            "linear-dodge"  => Some(Self::LinearDodge),
            "lighter-color" => Some(Self::LighterColor),
            "overlay"       => Some(Self::Overlay),
            "soft-light"    => Some(Self::SoftLight),
            "hard-light"    => Some(Self::HardLight),
            "vivid-light"   => Some(Self::VividLight),
            "linear-light"  => Some(Self::LinearLight),
            "pin-light"     => Some(Self::PinLight),
            "hard-mix"      => Some(Self::HardMix),
            "difference"    => Some(Self::Difference),
            "exclusion"     => Some(Self::Exclusion),
            "subtract"      => Some(Self::Subtract),
            "divide"        => Some(Self::Divide),
            "hue"           => Some(Self::Hue),
            "saturation"    => Some(Self::Saturation),
            "color"         => Some(Self::Color),
            "luminosity"    => Some(Self::Luminosity),
            _               => None,
        }
    }

    /// Return the AIF string identifier for this blend mode (SPEC.md §4.8).
    pub fn to_aif_str(self) -> &'static str {
        match self {
            Self::Normal       => "normal",
            Self::Dissolve     => "dissolve",
            Self::Darken       => "darken",
            Self::Multiply     => "multiply",
            Self::ColorBurn    => "color-burn",
            Self::LinearBurn   => "linear-burn",
            Self::DarkerColor  => "darker-color",
            Self::Lighten      => "lighten",
            Self::Screen       => "screen",
            Self::ColorDodge   => "color-dodge",
            Self::LinearDodge  => "linear-dodge",
            Self::LighterColor => "lighter-color",
            Self::Overlay      => "overlay",
            Self::SoftLight    => "soft-light",
            Self::HardLight    => "hard-light",
            Self::VividLight   => "vivid-light",
            Self::LinearLight  => "linear-light",
            Self::PinLight     => "pin-light",
            Self::HardMix      => "hard-mix",
            Self::Difference   => "difference",
            Self::Exclusion    => "exclusion",
            Self::Subtract     => "subtract",
            Self::Divide       => "divide",
            Self::Hue          => "hue",
            Self::Saturation   => "saturation",
            Self::Color        => "color",
            Self::Luminosity   => "luminosity",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_MODES: &[BlendMode] = &[
        BlendMode::Normal,   BlendMode::Dissolve,
        BlendMode::Darken,   BlendMode::Multiply,   BlendMode::ColorBurn,
        BlendMode::LinearBurn, BlendMode::DarkerColor,
        BlendMode::Lighten,  BlendMode::Screen,     BlendMode::ColorDodge,
        BlendMode::LinearDodge, BlendMode::LighterColor,
        BlendMode::Overlay,  BlendMode::SoftLight,  BlendMode::HardLight,
        BlendMode::VividLight, BlendMode::LinearLight, BlendMode::PinLight,
        BlendMode::HardMix,
        BlendMode::Difference, BlendMode::Exclusion, BlendMode::Subtract,
        BlendMode::Divide,
        BlendMode::Hue,      BlendMode::Saturation, BlendMode::Color,
        BlendMode::Luminosity,
    ];

    #[test]
    fn exactly_27_variants() {
        assert_eq!(ALL_MODES.len(), 27);
    }

    #[test]
    fn aif_str_round_trip() {
        for &mode in ALL_MODES {
            let s = mode.to_aif_str();
            assert_eq!(
                BlendMode::from_aif_str(s),
                Some(mode),
                "round-trip failed for {s:?}"
            );
        }
    }

    #[test]
    fn unknown_str_returns_none() {
        assert_eq!(BlendMode::from_aif_str("not-a-mode"), None);
        assert_eq!(BlendMode::from_aif_str("Normal"), None); // identifiers are lowercase
        assert_eq!(BlendMode::from_aif_str(""), None);
    }
}
