// Deep-space STARFIELD enclosure (we float in space). The box sits far out; its
// interior is painted with an animated procedural starfield by view direction —
// two parallax layers, per-star twinkle, and a slow drift — plus a faint nebula
// tint. This is the SAME star look the glass marbles refract (orb.wgsl), so the
// marbles show the space around them.
//
// WebGL2-safe: fragment-only, vec4 uniforms, no derivatives.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}

// rgb = nebula base tint (linear); a spare.
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// rgb = nebula accent tint (toward +Y); a spare.
@group(3) @binding(1) var<uniform> accent: vec4<f32>;
// x = star density, y = star brightness, z = twinkle amount, w = drift speed.
@group(3) @binding(2) var<uniform> space: vec4<f32>;
// x = nebula amount; yzw spare.
@group(3) @binding(3) var<uniform> space2: vec4<f32>;

fn hash13(p3: vec3<f32>) -> f32 {
    var p = fract(p3 * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

// One starfield shell: sparse twinkling points by direction.
fn star_layer(d: vec3<f32>, density: f32, bright: f32, twinkle: f32, t: f32) -> vec3<f32> {
    let uv = d * density;
    let cell = floor(uv);
    let f = fract(uv) - 0.5;
    let h = hash13(cell);
    let star = step(0.965, h);
    let b = hash13(cell + vec3<f32>(7.1, 2.3, 5.9));
    let pt = smoothstep(0.24, 0.0, length(f));
    // Per-star twinkle: each star pulses on its own phase/rate.
    let tw = 1.0 - twinkle * (0.5 + 0.5 * sin(t * (1.0 + b * 2.0) + b * 30.0));
    // White stars (no colour).
    return vec3<f32>(star * pt * (2.0 + b * 4.0) * bright * tw);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Starfield removed — just the up/down POLE NEBULAE on black space: a coloured
    // glow capping +Y and a different hue under -Y, black around the horizon where
    // the four steward signets sit. With the signets that gives all six directions.
    // (`space` / `star_layer` / `globals` left bound but unused.)
    let dir = normalize(in.world_position.xyz);
    let upcap = smoothstep(0.40, 1.0, dir.y);
    let downcap = smoothstep(0.40, 1.0, -dir.y);
    let nebula = (color.rgb * upcap + accent.rgb * downcap) * space2.x;
    return vec4<f32>(nebula, 1.0);
}
