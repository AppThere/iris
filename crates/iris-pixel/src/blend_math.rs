// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Per-pixel blend-mode mathematics for the 27 [`BlendMode`] variants
//! (SPEC.md §4.8).
//!
//! [`blend`] takes a backdrop and source colour — both straight (non
//! premultiplied) RGB triples in the compositor working space, nominal range
//! `[0, 1]` — and returns the blended colour `B(Cb, Cs)`. The caller is
//! responsible for the surrounding alpha composite (W3C "Compositing and
//! Blending Level 1"): `Cs' = (1 − αb)·Cs + αb·B(Cb, Cs)` then source-over.
//!
//! The separable formulas are the PDF/Photoshop blend equations; the four
//! non-separable modes (Hue/Saturation/Color/Luminosity) use the W3C
//! `SetLum`/`SetSat` helpers. Math is performed in whatever space the caller
//! supplies its components in.
//
// COMPAT(adobe): Photoshop's default blends in the document's gamma-encoded
// space, whereas the Iris compositor works in linear light (ADR-003). Results
// for contrast modes (Overlay, Soft/Hard Light, …) therefore differ slightly
// from Photoshop's default; this matches Photoshop's "Blend RGB Colors Using
// Gamma 1.0" option. A future gamma-space blend option is tracked separately.
// TODO(iris): SPEC.md §4.8 — optional gamma-space blend for exact PS parity.

use crate::BlendMode;

/// Blend a `source` colour over a `backdrop` colour for the given `mode`.
///
/// Both inputs are straight RGB triples in nominal `[0, 1]`; the result is the
/// blended colour `B(Cb, Cs)` (alpha compositing is the caller's job).
/// `Dissolve` is treated as `Normal` here — its stochastic behaviour is an
/// alpha effect applied by the compositor, not a colour blend.
// TODO(iris): SPEC.md §4.8 — Dissolve needs position-seeded stochastic alpha
// in the compositor; the colour function returns the source unchanged.
pub fn blend(mode: BlendMode, backdrop: [f32; 3], source: [f32; 3]) -> [f32; 3] {
    use BlendMode::*;
    match mode {
        Normal | Dissolve => source,
        Hue => set_lum(set_sat(source, sat(backdrop)), lum(backdrop)),
        Saturation => set_lum(set_sat(backdrop, sat(source)), lum(backdrop)),
        Color => set_lum(source, lum(backdrop)),
        Luminosity => set_lum(backdrop, lum(source)),
        DarkerColor => {
            if lum(source) <= lum(backdrop) {
                source
            } else {
                backdrop
            }
        }
        LighterColor => {
            if lum(source) >= lum(backdrop) {
                source
            } else {
                backdrop
            }
        }
        _ => [
            sep(mode, backdrop[0], source[0]),
            sep(mode, backdrop[1], source[1]),
            sep(mode, backdrop[2], source[2]),
        ],
    }
}

/// Separable (per-channel) blend. Non-separable modes are handled in [`blend`]
/// and never reach this function; they fall through to returning the source.
fn sep(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    use BlendMode::*;
    let out = match mode {
        Multiply => cb * cs,
        Screen => screen(cb, cs),
        Overlay => hard_light(cs, cb), // Overlay = HardLight with operands swapped
        Darken => cb.min(cs),
        Lighten => cb.max(cs),
        ColorDodge => color_dodge(cb, cs),
        ColorBurn => color_burn(cb, cs),
        HardLight => hard_light(cb, cs),
        SoftLight => soft_light(cb, cs),
        LinearBurn => cb + cs - 1.0,
        LinearDodge => cb + cs,
        LinearLight => cb + 2.0 * cs - 1.0,
        VividLight => vivid_light(cb, cs),
        PinLight => pin_light(cb, cs),
        HardMix => {
            if vivid_light(cb, cs) < 0.5 {
                0.0
            } else {
                1.0
            }
        }
        Difference => (cb - cs).abs(),
        Exclusion => cb + cs - 2.0 * cb * cs,
        Subtract => cb - cs,
        Divide => {
            if cs <= 0.0 {
                1.0
            } else {
                cb / cs
            }
        }
        _ => cs,
    };
    out.clamp(0.0, 1.0)
}

fn screen(cb: f32, cs: f32) -> f32 {
    cb + cs - cb * cs
}

fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        2.0 * cb * cs
    } else {
        screen(cb, 2.0 * cs - 1.0)
    }
}

fn soft_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
    } else {
        let d = if cb <= 0.25 {
            ((16.0 * cb - 12.0) * cb + 4.0) * cb
        } else {
            cb.sqrt()
        };
        cb + (2.0 * cs - 1.0) * (d - cb)
    }
}

fn color_dodge(cb: f32, cs: f32) -> f32 {
    if cb <= 0.0 {
        0.0
    } else if cs >= 1.0 {
        1.0
    } else {
        (cb / (1.0 - cs)).min(1.0)
    }
}

fn color_burn(cb: f32, cs: f32) -> f32 {
    if cb >= 1.0 {
        1.0
    } else if cs <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - cb) / cs).min(1.0)
    }
}

fn vivid_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        color_burn(cb, 2.0 * cs)
    } else {
        color_dodge(cb, 2.0 * cs - 1.0)
    }
}

fn pin_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb.min(2.0 * cs)
    } else {
        cb.max(2.0 * cs - 1.0)
    }
}

// --- Non-separable helpers (W3C Compositing and Blending Level 1) ----------

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// Clip a colour into `[0, 1]` while preserving its luminosity.
fn clip_color(mut c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    if n < 0.0 && (l - n).abs() > f32::EPSILON {
        for ch in &mut c {
            *ch = l + (*ch - l) * l / (l - n);
        }
    }
    if x > 1.0 && (x - l).abs() > f32::EPSILON {
        for ch in &mut c {
            *ch = l + (*ch - l) * (1.0 - l) / (x - l);
        }
    }
    c
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// Set the saturation of `c` to `s`, preserving relative channel ordering.
fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    // Indices of the min, mid, and max channels.
    let (mut imin, mut imid, mut imax) = (0usize, 1usize, 2usize);
    if c[imin] > c[imid] {
        std::mem::swap(&mut imin, &mut imid);
    }
    if c[imid] > c[imax] {
        std::mem::swap(&mut imid, &mut imax);
    }
    if c[imin] > c[imid] {
        std::mem::swap(&mut imin, &mut imid);
    }
    let mut out = [0.0_f32; 3];
    if c[imax] > c[imin] {
        out[imid] = (c[imid] - c[imin]) * s / (c[imax] - c[imin]);
        out[imax] = s;
    }
    // out[imin] stays 0.0; mid/max already set (or 0 when max == min).
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: [f32; 3] = [0.5, 0.5, 0.5];

    fn approx(a: [f32; 3], b: [f32; 3]) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-4, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn normal_returns_source() {
        approx(blend(BlendMode::Normal, [0.2, 0.4, 0.6], G), G);
    }

    #[test]
    fn separable_known_values() {
        approx(blend(BlendMode::Multiply, G, G), [0.25; 3]);
        approx(blend(BlendMode::Screen, G, G), [0.75; 3]);
        approx(blend(BlendMode::Difference, [0.7; 3], [0.2; 3]), [0.5; 3]);
        approx(blend(BlendMode::Darken, [0.7; 3], [0.2; 3]), [0.2; 3]);
        approx(blend(BlendMode::Lighten, [0.7; 3], [0.2; 3]), [0.7; 3]);
        // Overlay/HardLight of 0.5 over 0.5 is the identity midpoint.
        approx(blend(BlendMode::Overlay, G, G), G);
        approx(blend(BlendMode::HardLight, G, G), G);
    }

    #[test]
    fn dodge_burn_extremes() {
        approx(blend(BlendMode::ColorDodge, [0.5; 3], [1.0; 3]), [1.0; 3]);
        approx(blend(BlendMode::ColorBurn, [0.5; 3], [0.0; 3]), [0.0; 3]);
        approx(blend(BlendMode::HardMix, [0.9; 3], [0.9; 3]), [1.0; 3]);
        approx(blend(BlendMode::HardMix, [0.1; 3], [0.1; 3]), [0.0; 3]);
    }

    #[test]
    fn luminosity_takes_source_luma_backdrop_chroma() {
        // Luminosity keeps the backdrop hue/sat but adopts the source luma.
        let backdrop = [0.8, 0.2, 0.2];
        let source = [0.5, 0.5, 0.5];
        let out = blend(BlendMode::Luminosity, backdrop, source);
        assert!((lum(out) - lum(source)).abs() < 1e-3);
    }

    #[test]
    fn color_takes_backdrop_luma() {
        let backdrop = [0.3, 0.3, 0.3];
        let source = [0.1, 0.6, 0.9];
        let out = blend(BlendMode::Color, backdrop, source);
        assert!((lum(out) - lum(backdrop)).abs() < 1e-3);
    }

    #[test]
    fn all_modes_stay_finite_and_in_range() {
        use BlendMode::*;
        let modes = [
            Normal, Dissolve, Darken, Multiply, ColorBurn, LinearBurn, DarkerColor,
            Lighten, Screen, ColorDodge, LinearDodge, LighterColor, Overlay, SoftLight,
            HardLight, VividLight, LinearLight, PinLight, HardMix, Difference, Exclusion,
            Subtract, Divide, Hue, Saturation, Color, Luminosity,
        ];
        assert_eq!(modes.len(), 27);
        let samples = [[0.0; 3], [1.0; 3], [0.25, 0.5, 0.75], [0.9, 0.1, 0.4]];
        for m in modes {
            for &cb in &samples {
                for &cs in &samples {
                    let o = blend(m, cb, cs);
                    for v in o {
                        assert!(v.is_finite() && (-1e-3..=1.0001).contains(&v),
                            "mode {m:?} cb {cb:?} cs {cs:?} -> {o:?}");
                    }
                }
            }
        }
    }
}
