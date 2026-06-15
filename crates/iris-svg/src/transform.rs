// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Parse and serialise SVG `transform` lists as kurbo [`Affine`] matrices.

use kurbo::Affine;

/// Parse an SVG `transform` attribute into a single composed [`Affine`].
///
/// Supports `matrix`, `translate`, `scale`, `rotate`, `skewX`, and `skewY`.
/// Functions compose left-to-right (the leftmost is the outermost transform).
/// Unrecognised functions are ignored.
pub(crate) fn parse(s: &str) -> Affine {
    let mut result = Affine::IDENTITY;
    let mut rest = s.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim();
        let Some(close) = rest[open + 1..].find(')') else {
            break;
        };
        let args = parse_args(&rest[open + 1..open + 1 + close]);
        if let Some(m) = func_to_affine(name, &args) {
            result *= m;
        }
        rest = rest[open + 1 + close + 1..].trim_start_matches([',', ' ', '\t', '\n', '\r']);
    }
    result
}

fn parse_args(s: &str) -> Vec<f64> {
    s.split([',', ' ', '\t', '\n', '\r'])
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

fn func_to_affine(name: &str, a: &[f64]) -> Option<Affine> {
    match name {
        "matrix" if a.len() == 6 => Some(Affine::new([a[0], a[1], a[2], a[3], a[4], a[5]])),
        "translate" if a.len() == 1 => Some(Affine::translate((a[0], 0.0))),
        "translate" if a.len() >= 2 => Some(Affine::translate((a[0], a[1]))),
        "scale" if a.len() == 1 => Some(Affine::scale(a[0])),
        "scale" if a.len() >= 2 => Some(Affine::scale_non_uniform(a[0], a[1])),
        "rotate" if a.len() == 1 => Some(Affine::rotate(a[0].to_radians())),
        "rotate" if a.len() >= 3 => Some(
            Affine::translate((a[1], a[2]))
                * Affine::rotate(a[0].to_radians())
                * Affine::translate((-a[1], -a[2])),
        ),
        "skewX" if !a.is_empty() => Some(Affine::new([1.0, 0.0, a[0].to_radians().tan(), 1.0, 0.0, 0.0])),
        "skewY" if !a.is_empty() => Some(Affine::new([1.0, a[0].to_radians().tan(), 0.0, 1.0, 0.0, 0.0])),
        _ => None,
    }
}

/// Serialise an affine as an SVG `matrix(...)`, or `None` if it is the identity.
pub(crate) fn to_svg(a: Affine) -> Option<String> {
    if a == Affine::IDENTITY {
        return None;
    }
    let c = a.as_coeffs();
    Some(format!("matrix({} {} {} {} {} {})", c[0], c[1], c[2], c[3], c[4], c[5]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;

    #[test]
    fn translate_then_scale_composes_left_to_right() {
        let a = parse("translate(10,20) scale(2)");
        let p = a * Point::new(1.0, 1.0);
        assert_eq!(p, Point::new(12.0, 22.0));
    }

    #[test]
    fn matrix_round_trips() {
        let a = parse("matrix(1 0 0 1 5 6)");
        assert_eq!(a, Affine::translate((5.0, 6.0)));
        assert!(to_svg(a).unwrap().starts_with("matrix("));
        assert_eq!(to_svg(Affine::IDENTITY), None);
    }
}
