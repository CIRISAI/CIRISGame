// Steward orb (DESIGN_BRIEF §3.2/§3.3): one sphere that reads as a thick CLEAR
// glass shell with TWO swirling gasses inside it — a faint clear gas and a bright
// NEON coloured gas — composited in a single transparent surface so neither
// occludes the other (a separate glass shell would depth-occlude the gas).
//
// The "thick clear glass" is faked with a strong, wide Fresnel rim (edge catch)
// plus a mostly-transparent centre you see through. The neon gas is pushed deep
// into HDR so Bloom turns the wisps into a hot glow. WebGL2-safe: fragment-only,
// two smooth value-noise samples, vec4 uniforms, alpha-blended, no derivatives.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}

// rgb = neon pigment (linear); a = base centre opacity.
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// x = swirl speed; y = swirl scale; z = neon glow gain; w = rim (glass) gain.
@group(3) @binding(1) var<uniform> params: vec4<f32>;
// xyz = cursor focus point (world); w = selection strength [0,1].
@group(3) @binding(2) var<uniform> hover: vec4<f32>;

fn hash13(p3: vec3<f32>) -> f32 {
    var p = fract(p3 * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

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

// A swirling value-noise field: rotate the sample domain about Y over time so the
// gas churns. `ph` offsets the two gasses so they swirl independently.
fn gas(p: vec3<f32>, t: f32, ph: f32) -> f32 {
    let a = t + ph;
    let s = sin(a);
    let c = cos(a);
    let q = vec3<f32>(p.x * c - p.z * s, p.y + a * 0.4, p.x * s + p.z * c);
    return vnoise(q + ph);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let speed = params.x;
    let scale = params.y;
    let neon_glow = params.z;
    let rim_gain = params.w;

    let p = in.world_position.xyz * scale;
    let t = globals.time * speed;

    // Two independent swirling gasses.
    let neon_f = gas(p, t, 0.0);
    let clear_f = gas(p * 1.4, t * 0.6, 11.7);

    // Neon gas: the steward colour glows across the whole interior (so it reads as
    // NEON, not a faint tint), hottest along the swirl wisps. Deep in HDR → bloom.
    let wisp = smoothstep(0.35, 0.8, neon_f);
    let neon = color.rgb * (neon_glow * (0.4 + 1.1 * wisp));

    // Clear gas: a faint cool-white haze, a second swirling medium.
    let haze = smoothstep(0.4, 0.95, clear_f);
    let clear = vec3<f32>(0.85, 0.92, 1.0) * (haze * 0.4);

    // Thick clear glass: a wide, bright Fresnel rim catching light at the
    // silhouette (low exponent = wider band = thicker-looking glass), tinted
    // toward white so it reads as a glass edge.
    let nrm = normalize(in.world_normal);
    let view_dir = normalize(view.world_position.xyz - in.world_position.xyz);
    let fres = pow(1.0 - clamp(dot(nrm, view_dir), 0.0, 1.0), 1.8);
    let rim = mix(color.rgb, vec3<f32>(1.0), 0.4) * (rim_gain * fres);

    // Selection: when the cursor is on this sphere, it swirls with extra light —
    // the wisps brighten and the whole orb lifts toward white (reads as "picked").
    let sel = hover.w * exp(-distance(in.world_position.xyz, hover.xyz) * 1.6);
    let sel_light = mix(color.rgb, vec3<f32>(1.0), 0.4)
        * (sel * (1.5 + 3.0 * wisp));

    let col = neon + clear + rim + sel_light;
    // See-through centre (clear glass); denser at the neon wisps + glass rim, and
    // firmer when selected.
    let alpha = clamp(
        color.a * 0.3 + wisp * 0.5 + haze * 0.15 + fres * 0.7 + sel * 0.4,
        0.0,
        1.0,
    );
    return vec4<f32>(col, alpha);
}
