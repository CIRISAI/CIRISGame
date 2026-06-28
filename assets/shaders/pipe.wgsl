// Liquid-pigment pipe (DESIGN_BRIEF §3.4).
//
// A fragment shader that fills the glass channel with the steward's pigment as a
// liquid: the surface pools toward world-down (gravity), tilts when the camera
// orbits (a CPU slosh impulse fed in through `slosh`), and scrolls a noise field
// so the column reads as moving liquid. WebGL2-safe by construction — no compute,
// no texture sampling, a constant 2-octave noise loop, vec4-aligned uniforms, and
// `discard` only (no derivatives after it). Validated through naga to GLSL ES 300,
// exactly like `mist.wgsl`. Material bind group is @group(3) for 3D materials in
// Bevy 0.19.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

// rgb = steward pigment (linear); a = liquid opacity.
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// xyz = pipe world centre; w = vertical half-extent (along world up).
@group(3) @binding(1) var<uniform> geom: vec4<f32>;
// x = fill (0..1); y = flow speed; z = noise freq; w = slosh strength.
@group(3) @binding(2) var<uniform> dynamics: vec4<f32>;
// xyz = slosh displacement (world space, lateral); w = unused.
@group(3) @binding(3) var<uniform> slosh: vec4<f32>;

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

// Two-octave fbm — the moving liquid surface + body texture.
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
    let c = geom.xyz;
    let extent = max(geom.w, 1.0e-3);
    let rel = in.world_position.xyz - c;

    let fill = dynamics.x;
    let flow_speed = dynamics.y;
    let freq = dynamics.z;
    let slosh_strength = dynamics.w;

    // Liquid "up" = world-up tilted by the slosh displacement; at rest the surface
    // is level and the liquid pools toward world-down (gravity).
    let up = normalize(vec3<f32>(0.0, 1.0, 0.0) + slosh.xyz * slosh_strength);
    // Signed height of this fragment above the pipe centre, normalized to ~[-1, 1].
    let h = dot(rel, up) / extent;

    // Animated surface: scroll a noise field so the column reads as moving liquid,
    // and let it ripple the surface line a little.
    let drift = vec3<f32>(globals.time * flow_speed, globals.time * flow_speed * 0.6, 0.0);
    let surf_noise = fbm(rel * freq + drift);
    let surface = (fill * 2.0 - 1.0) + (surf_noise - 0.5) * 0.16;

    // Above the surface is air — carve it away so the tube reads as partly full.
    if (h > surface) {
        discard;
    }

    // Depth below the surface drives the body shade; a bright meniscus band sits
    // right under the surface, the body deepens toward the bottom.
    let depth = clamp(surface - h, 0.0, 1.0);
    let body = fbm(rel * (freq * 0.5) + drift * 0.5);
    let meniscus = smoothstep(0.12, 0.0, depth);
    let shade = 0.5 + 0.45 * body + 0.45 * meniscus;
    return vec4<f32>(color.rgb * shade, color.a);
}
