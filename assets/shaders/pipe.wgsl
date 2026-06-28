// Gas-pigment pipe (DESIGN_BRIEF §3.4).
//
// A soft luminous gas of the steward's pigment drifting through the glass channel
// between connected cells. Translucent, slowly churning, no physics (no gravity
// fill, no slosh). WebGL2-safe by construction — no compute, no texture sampling,
// a constant 2-octave noise loop, vec4-aligned uniforms, alpha-blended (no
// derivatives), validated through naga to GLSL ES 300 like mist.wgsl.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

// rgb = steward pigment (linear); a = gas opacity.
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// x = flow speed; y = noise freq; z = density gain; w = unused.
@group(3) @binding(1) var<uniform> params: vec4<f32>;

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

// Two-octave fbm — the drifting gas body.
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
    let speed = params.x;
    let freq = params.y;
    let density = params.z;

    // Slow gaseous churn: drift the noise field along all three axes over time.
    let drift = vec3<f32>(
        globals.time * speed,
        globals.time * speed * 0.7,
        globals.time * speed * 0.5,
    );
    let n = fbm(in.world_position.xyz * freq + drift);

    // Density-modulated translucency: thin wisps fade out, dense pockets glow.
    let d = clamp(n * density, 0.0, 1.0);
    let alpha = color.a * smoothstep(0.12, 0.85, d);
    let glow = 0.55 + 0.75 * d;
    return vec4<f32>(color.rgb * glow, alpha);
}
