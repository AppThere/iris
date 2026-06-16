// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! CSS colour parsing and serialisation for SVG paint values.

use iris_vector::Color;

/// Parse a CSS colour token (`#rgb`, `#rrggbb`, `rgb(r,g,b)`, or a basic named
/// colour). Returns `None` for `none`, `transparent`, or unrecognised values.
pub(crate) fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        return parse_rgb_fn(inner);
    }
    named(s)
}

fn parse_hex(hex: &str) -> Option<Color> {
    match hex.len() {
        3 => {
            let r = dup_nibble(hex.as_bytes()[0])?;
            let g = dup_nibble(hex.as_bytes()[1])?;
            let b = dup_nibble(hex.as_bytes()[2])?;
            Some(Color::from_rgb8(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::from_rgb8(r, g, b))
        }
        _ => None,
    }
}

fn dup_nibble(c: u8) -> Option<u8> {
    let v = (c as char).to_digit(16)? as u8;
    Some(v << 4 | v)
}

fn parse_rgb_fn(inner: &str) -> Option<Color> {
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let comp = |p: &str| -> Option<u8> {
        let p = p.trim();
        if let Some(pct) = p.strip_suffix('%') {
            let v: f32 = pct.trim().parse().ok()?;
            Some((v / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8)
        } else {
            p.parse::<f32>().ok().map(|v| v.round().clamp(0.0, 255.0) as u8)
        }
    };
    Some(Color::from_rgb8(comp(parts[0])?, comp(parts[1])?, comp(parts[2])?))
}

fn named(s: &str) -> Option<Color> {
    let c = |r, g, b| Some(Color::from_rgb8(r, g, b));
    match s.to_ascii_lowercase().as_str() {
        "black" => c(0, 0, 0),
        "white" => c(255, 255, 255),
        "red" => c(255, 0, 0),
        "green" => c(0, 128, 0),
        "lime" => c(0, 255, 0),
        "blue" => c(0, 0, 255),
        "yellow" => c(255, 255, 0),
        "cyan" | "aqua" => c(0, 255, 255),
        "magenta" | "fuchsia" => c(255, 0, 255),
        "gray" | "grey" => c(128, 128, 128),
        "silver" => c(192, 192, 192),
        "maroon" => c(128, 0, 0),
        "olive" => c(128, 128, 0),
        "navy" => c(0, 0, 128),
        "purple" => c(128, 0, 128),
        "teal" => c(0, 128, 128),
        "orange" => c(255, 165, 0),
        _ => None,
    }
}

/// Serialise a colour as `#rrggbb` (alpha is carried separately as opacity).
pub(crate) fn to_hex(c: Color) -> String {
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", to8(c.r), to8(c.g), to8(c.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_forms() {
        assert_eq!(parse_color("#f00"), Some(Color::from_rgb8(255, 0, 0)));
        assert_eq!(parse_color("#00ff00"), Some(Color::from_rgb8(0, 255, 0)));
    }

    #[test]
    fn parses_rgb_and_named() {
        assert_eq!(parse_color("rgb(0,0,255)"), Some(Color::from_rgb8(0, 0, 255)));
        assert_eq!(parse_color("blue"), Some(Color::from_rgb8(0, 0, 255)));
        assert_eq!(parse_color("none"), None);
    }

    #[test]
    fn hex_round_trip() {
        let c = Color::from_rgb8(18, 52, 86);
        assert_eq!(to_hex(c), "#123456");
        assert_eq!(parse_color(&to_hex(c)), Some(c));
    }
}
