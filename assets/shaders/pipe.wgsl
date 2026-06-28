// Swirling-gas material (DESIGN_BRIEF §3.3/§3.4).
//
// A solid-colour steward gas that slowly SWIRLS — used both for the gas inside a
// live sphere's clear glass shell and for the fat pipe between two connected
// same-colour spheres. Deliberately a *solid colour* with gentle swirling motion
// (not a busy noise pattern — patterns read as distracting texture). WebGL2-safe:
// fragment-only, one smooth value-noise sample, constant loop bounds, vec4
// uniforms, alpha-blended, no derivatives.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}

// rgb = steward pigment (linear); a = peak opacity.
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// x = swirl speed; y = swirl scale; z = glow gain (HDR>1 → bloom); w = solidity.
@group(3) @binding(1) var<uniform> params: vec4<f32>;

// Cheap 3D hash → scalar in [0, 1] (Dave Hoskins style), webgl2-safe.
fn hash13(p3: vec3<f32>) -> f32 {
    var p = fract(p3 * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

// Trilinearly-interpolated value noise (one smooth octave).
fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let c000 = hash13(i + vec3<f32>(0.0, 0.0, 0.0));
    let c100 = hash13(i + vec3<f32>(1.0, 0.0, 0.0));
    let c010 = hash13(i + vec3<f32>(0.0, 1.0, 0.0));
    let c110 = hash13(i + vec3<f32>(1.0, 1.0, 0.0));
    let c001 = hash13(i + vec3<f32>(0.0, 0.0, 1.0));
    let c101 = hash13(i + vec3<f32>(1.0, 0.0, 1.0));
    let c011 = hash13(i + vec3<f32>(0.0, 1.0, 1.0));
    let c111 = hash13(i + vec3<f32>(1.0, 1.0, 1.0));
    let x00 = mix(c000, c100, u.x);
    let x10 = mix(c010, c110, u.x);
    let x01 = mix(c001, c101, u.x);
    let x11 = mix(c011, c111, u.x);
    let y0 = mix(x00, x10, u.y);
    let y1 = mix(x01, x11, u.y);
    return mix(y0, y1, u.z);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let speed = params.x;
    let scale = params.y;
    let glow = params.z;
    let solidity = params.w;

    // Swirl: rotate the sample domain about Y over time + a slow vertical drift,
    // so a single smooth noise sample reads as gently churning gas rather than a
    // fixed texture.
    let a = globals.time * speed;
    let s = sin(a);
    let c = cos(a);
    let p = in.world_position.xyz * scale;
    let q = vec3<f32>(p.x * c - p.z * s, p.y + a * 0.5, p.x * s + p.z * c);
    let n = vnoise(q);

    // Mostly solid colour: `solidity` pulls the field toward a uniform fill so the
    // swirl only nudges brightness/opacity instead of painting a pattern.
    let d = mix(n, 0.72, solidity);
    let body = glow * (0.72 + 0.5 * d);

    // Fresnel rim: brighten + thicken the silhouette so the gas reads as a
    // volumetric glowing orb (and the pipe as a rounded tube), not a flat fill.
    // Pushed into HDR so Bloom blooms the edges — the "hot" glow.
    let nrm = normalize(in.world_normal);
    let view_dir = normalize(view.world_position.xyz - in.world_position.xyz);
    let fres = pow(1.0 - clamp(dot(nrm, view_dir), 0.0, 1.0), 3.0);
    let rim = fres * glow * 1.7;

    let alpha = clamp(color.a * (0.5 + 0.5 * d) + fres * 0.45, 0.0, 1.0);
    return vec4<f32>(color.rgb * (body + rim), alpha);
}
