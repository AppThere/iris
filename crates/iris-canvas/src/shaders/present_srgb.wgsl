// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0
//
// Present pass: convert the premultiplied linear-light Rgba16Float composite
// into the straight-alpha sRGB Rgba8Unorm texture that Vello's image atlas
// expects (same convention as the CPU path's LUT conversion).

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle: vertex_index 0..3 covers the whole target.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VOut;
    out.pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    // NDC y is up, texture v is down: flip so source and target align 1:1.
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

fn srgb_encode(x: f32) -> f32 {
    if x <= 0.0031308 {
        return 12.92 * x;
    }
    return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(t_src, s_src, in.uv);
    let a = clamp(c.a, 0.0, 1.0);
    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    if a > 0.0 {
        // Premultiplied → straight alpha.
        rgb = clamp(c.rgb / a, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    return vec4<f32>(srgb_encode(rgb.r), srgb_encode(rgb.g), srgb_encode(rgb.b), a);
}
