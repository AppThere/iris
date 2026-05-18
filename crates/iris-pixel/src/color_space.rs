// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`ColorSpaceId`] newtype and well-known colour space constants.
//!
//! String identifiers follow SPEC.md §4.9. The working colour space for all
//! compositor operations is [`LINEAR_SRGB`] (f16 linear light, per ADR-003).

/// Identifier for a pixel-data colour space; wraps a `&'static str` for
/// zero-cost equality comparison and deterministic serialisation.
///
/// Use the provided constants ([`SRGB`], [`LINEAR_SRGB`], etc.) rather than
/// constructing this type directly. Unknown colour spaces from third-party
/// AIF files should be parsed via [`ColorSpaceId::from_aif_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorSpaceId(&'static str);

impl ColorSpaceId {
    /// Return the AIF string identifier for this colour space (SPEC.md §4.9).
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// Parse a colour space identifier from an AIF file string.
    ///
    /// Returns `None` for unknown identifiers; callers should warn and fall
    /// back to [`SRGB`] rather than failing hard.
    pub fn from_aif_str(s: &str) -> Option<Self> {
        match s {
            "srgb"          => Some(SRGB),
            "linear-srgb"   => Some(LINEAR_SRGB),
            "display-p3"    => Some(DISPLAY_P3),
            "prophoto-rgb"  => Some(PROPHOTO_RGB),
            "cmyk-generic"  => Some(CMYK_GENERIC),
            _               => None,
        }
    }

    /// Returns `true` if this colour space can be used with the given channel layout.
    ///
    /// [`CMYK_GENERIC`] is only compatible with [`ChannelLayout::Cmyk`]; all other
    /// colour spaces require an RGB-family layout. Passing an incompatible combination
    /// produces a visual warning overlay in the compositor (ADR-003).
    pub fn is_compatible_with_channel_layout(
        self,
        layout: crate::pixel_layer::ChannelLayout,
    ) -> bool {
        use crate::pixel_layer::ChannelLayout;
        let is_cmyk_space = self == CMYK_GENERIC;
        let is_cmyk_layout = matches!(layout, ChannelLayout::Cmyk);
        // CMYK space ↔ CMYK layout must match; mixing them is an error.
        is_cmyk_space == is_cmyk_layout
    }
}

/// Standard sRGB (gamma-encoded, display-referred).
pub const SRGB: ColorSpaceId = ColorSpaceId("srgb");
/// Linear-light sRGB — the compositor's working colour space (ADR-003).
pub const LINEAR_SRGB: ColorSpaceId = ColorSpaceId("linear-srgb");
/// DCI-P3 with a D65 white point (wide-gamut display-referred).
pub const DISPLAY_P3: ColorSpaceId = ColorSpaceId("display-p3");
/// ProPhoto RGB (very wide gamut; linear-light encoding recommended).
pub const PROPHOTO_RGB: ColorSpaceId = ColorSpaceId("prophoto-rgb");
/// Generic CMYK — composited via ICC profile LUT (Phase 4 only; ADR-003).
///
/// Layers using this colour space produce a visible warning overlay in the
/// compositor until the `appthere-color` CMYK extension is wired (Phase 4).
// TODO(iris): SPEC.md §4.9 — Phase 4: replace warning overlay with real ICC LUT compositing.
pub const CMYK_GENERIC: ColorSpaceId = ColorSpaceId("cmyk-generic");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pixel_layer::ChannelLayout;

    #[test]
    fn round_trip_all_known() {
        let known = [SRGB, LINEAR_SRGB, DISPLAY_P3, PROPHOTO_RGB, CMYK_GENERIC];
        for cs in known {
            assert_eq!(
                ColorSpaceId::from_aif_str(cs.as_str()),
                Some(cs),
                "round-trip failed for {:?}",
                cs.as_str()
            );
        }
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(ColorSpaceId::from_aif_str("xyz"), None);
        assert_eq!(ColorSpaceId::from_aif_str(""), None);
    }

    #[test]
    fn cmyk_compatibility() {
        assert!(CMYK_GENERIC.is_compatible_with_channel_layout(ChannelLayout::Cmyk));
        assert!(!CMYK_GENERIC.is_compatible_with_channel_layout(ChannelLayout::Rgba));
        assert!(!SRGB.is_compatible_with_channel_layout(ChannelLayout::Cmyk));
        assert!(SRGB.is_compatible_with_channel_layout(ChannelLayout::Rgba));
        assert!(LINEAR_SRGB.is_compatible_with_channel_layout(ChannelLayout::L));
    }
}
