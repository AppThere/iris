// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Flatten the layer tree into a single canvas-sized RGBA buffer for the PSD
//! merged image-data section (the composite Photoshop shows without parsing
//! layers).

use iris_aif::layer_to_rgba8;
use iris_pixel::LayerTree;

/// Composite all visible pixel layers bottom-to-top into a straight-alpha 8-bit
/// sRGB buffer of `width * height * 4` bytes.
///
/// Compositing is done in sRGB space with Porter-Duff "over". Group-level
/// opacity/blend are not applied (group nodes carry no pixels); non-Normal
/// blend modes fall back to "over".
// TODO(iris): SPEC.md §6.2 — composite in linear light and honour group/blend
// modes once the shared compositor is reusable outside iris-canvas.
pub(super) fn flatten(tree: &LayerTree, width: u32, height: u32) -> Vec<u8> {
    let mut canvas = vec![0u8; width as usize * height as usize * 4];

    // iter_depth_first is top-first; composite bottom-to-top.
    let layers: Vec<_> = tree.iter_depth_first().collect();
    for layer in layers.iter().rev() {
        if !layer.visible {
            continue;
        }
        let Some(px) = layer_to_rgba8(layer) else {
            continue; // groups and non-pixel layers contribute no pixels
        };
        let layer_alpha = layer.opacity.clamp(0.0, 1.0);

        for y in 0..px.height {
            let cy = px.offset_y + y as i32;
            if cy < 0 || cy >= height as i32 {
                continue;
            }
            for x in 0..px.width {
                let cx = px.offset_x + x as i32;
                if cx < 0 || cx >= width as i32 {
                    continue;
                }
                let si = ((y * px.width + x) * 4) as usize;
                let di = ((cy as u32 * width + cx as u32) * 4) as usize;
                over(&px.rgba[si..si + 4], &mut canvas[di..di + 4], layer_alpha);
            }
        }
    }
    canvas
}

/// Porter-Duff "over": composite straight-alpha `src` (scaled by `opacity`)
/// onto straight-alpha `dst` in place.
fn over(src: &[u8], dst: &mut [u8], opacity: f32) {
    let sa = (src[3] as f32 / 255.0) * opacity;
    if sa <= 0.0 {
        return;
    }
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    for c in 0..3 {
        let s = src[c] as f32 / 255.0;
        let d = dst[c] as f32 / 255.0;
        let out = (s * sa + d * da * (1.0 - sa)) / out_a;
        dst[c] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}
