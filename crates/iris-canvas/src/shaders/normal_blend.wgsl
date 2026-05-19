// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0
//
// Normal-blend render shader for iris-canvas Phase 2.
//
// Design note: the audit specified a compute shader with
// texture_storage_2d<rgba16float, read_write>, but wgpu 26 only supports
// read_write storage access for rgba16float with TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
// which is not universally available. Using a render pipeline with
// wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING gives identical results for
// Normal blend mode with no feature flag.
//
// TODO(iris): SPEC.md §4.8 — Phase 4: replace with compute pipeline for
// non-Normal blend modes (SHADER_F16 + compute are universally supported on
// all five target platforms per the audit).

struct TileParams {
    // NDC coordinates of the four tile corners on screen.
    // Passed as a pair of corners; bilinear interpolation handles rotation.
    corner_tl: vec2<f32>,   // top-left  in NDC
    corner_tr: vec2<f32>,   // top-right in NDC
    corner_bl: vec2<f32>,   // bottom-left  in NDC
    corner_br: vec2<f32>,   // bottom-right in NDC
    opacity: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var t_tile: texture_2d<f32>;
@group(0) @binding(1) var s_tile: sampler;
@group(0) @binding(2) var<uniform> params: TileParams;

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Vertex UVs: index → (u, v) in [0,1] x [0,1]
// 0 = TL (0,0), 1 = TR (1,0), 2 = BL (0,1), 3 = BR (1,1)
// Draw call: draw(0..4) with TriangleStrip topology.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    let u = f32(vi & 1u);
    let v = f32((vi >> 1u) & 1u);
    // Bilinear interpolation of the four NDC corners.
    // Correct for both axis-aligned and rotated tiles.
    let top    = mix(params.corner_tl, params.corner_tr, u);
    let bottom = mix(params.corner_bl, params.corner_br, u);
    let pos    = mix(top, bottom, v);
    var out: VertexOut;
    out.clip_pos = vec4<f32>(pos, 0.0, 1.0);
    out.uv       = vec2<f32>(u, v);
    return out;
}

// Q3 decision: premultiply straight-alpha on the fly in the fragment shader.
// The render pipeline blend state (PREMULTIPLIED_ALPHA_BLENDING) then applies
// Porter-Duff "over" in hardware:
//   out_rgb = src_rgb + dst_rgb * (1 - src_a)
//   out_a   = src_a   + dst_a   * (1 - src_a)
@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let c = textureSample(t_tile, s_tile, in.uv);
    // Premultiply straight alpha, then scale by layer opacity.
    return vec4<f32>(c.rgb * c.a, c.a) * params.opacity;
}
