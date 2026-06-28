// Dead-group volumetric mist (DESIGN_BRIEF §3.6 / §4.6).
//
// A fragment-shader raymarch through a per-cell bounding sphere: 32 fixed steps,
// 3D value-noise fbm (2 octaves, base freq from the uniform), flowing along +Y at
// the steward-driven speed. WebGL2-safe by construction — no compute, no texture
// sampling, a constant loop bound, and `discard` only at the very end (uniform
// control flow), so the GLES3 backend accepts it. `AlphaMode::Opaque` means the
// softness comes from the discard-on-low-density carve, not alpha blending; the
// mist renders in the opaque phase so the transmissive glass shell (§3.2)
// refracts it, exactly like the emissive cores.
//
// Material bind group is @group(3) for 3D materials in Bevy 0.19.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{view, globals}

// rgb = mist tint; a = the §4.6 appear/fade factor in [0, 1] (scales density).
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// x = flow speed (units/s), y = noise frequency, z = sphere radius, w = unused.
@group(3) @binding(1) var<uniform> flow: vec4<f32>;
// xyz = cell world centre, w = unused.
@group(3) @binding(2) var<uniform> center: vec4<f32>;

// Cheap 3D hash → scalar in [0, 1] (Dave Hoskins style), webgl2-safe.
fn hash13(p3: vec3<f32>) -> f32 {
    var p = fract(p3 * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

// Trilinearly-interpolated value noise.
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

// Two-octave fbm (§3.6: octave 2).
fn fbm(p0: vec3<f32>) -> f32 {
    var p = p0;
    var n = 0.0;
    var a = 0.6;
    for (var o = 0; o < 2; o = o + 1) {
        n += a * vnoise(p);
        p = p * 2.02 + vec3<f32>(17.1, 9.2, 31.7);
        a *= 0.5;
    }
    return n;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = center.xyz;
    let r = flow.z;

    // View ray from the camera through this fragment.
    let ro = view.world_position.xyz;
    let rd = normalize(in.world_position.xyz - ro);

    // Ray vs. the cell's bounding sphere — march only the interior segment.
    let oc = ro - c;
    let b = dot(oc, rd);
    let cc = dot(oc, oc) - r * r;
    let h = b * b - cc;
    if (h < 0.0) {
        discard;
    }
    let sh = sqrt(h);
    let t0 = max(-b - sh, 0.0);
    let t1 = -b + sh;

    let flow_speed = flow.x;
    let freq = flow.y;
    let drift = vec3<f32>(0.0, -globals.time * flow_speed, globals.time * flow_speed * 0.15);

    // 32-step accumulation of fbm density with a radial edge falloff.
    let steps = 32;
    let dt = (t1 - t0) / f32(steps);
    var density = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let t = t0 + (f32(i) + 0.5) * dt;
        let p = ro + rd * t;
        let q = (p - c) * freq + drift;
        let n = fbm(q);
        let rad = length(p - c) / r;
        let fall = smoothstep(1.0, 0.25, rad);
        density += max(n - 0.5, 0.0) * fall * dt;
    }
    density = density * 7.0;

    // `color.a` carries the appear/fade factor, so dissolving lowers density and
    // the discard carves the mist away (§4.6 fades).
    let amount = density * color.a;
    if (amount < 0.05) {
        discard;
    }

    // Denser core reads brighter; edges of kept fragments stay dim — wispy volume.
    let shade = 0.45 + 0.9 * clamp(density, 0.0, 1.0);
    return vec4<f32>(color.rgb * shade, 1.0);
}
