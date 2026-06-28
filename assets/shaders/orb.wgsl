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
// x = gas saturation (live-tuned); yzw spare.
@group(3) @binding(3) var<uniform> params2: vec4<f32>;

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

    let wisp = smoothstep(0.35, 0.8, neon_f);
    let haze = smoothstep(0.4, 0.95, clear_f);

    let nrm = normalize(in.world_normal);
    let view_dir = normalize(view.world_position.xyz - in.world_position.xyz);
    let fres = pow(1.0 - clamp(dot(nrm, view_dir), 0.0, 1.0), 1.8);
    // Selection: the sphere under the cursor swirls with extra light.
    let sel = hover.w * exp(-distance(in.world_position.xyz, hover.xyz) * 1.6);

    if (color.a >= 0.999) {
        // OPAQUE LIVE CORE → push SATURATION (not brightness) so the muted
        // steward pigment reads as neon without blooming to white, which would
        // also erase the glass edge. Saturate by pushing away from luminance.
        let lum = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        let sat = max(vec3<f32>(0.0), vec3<f32>(lum) + (color.rgb - vec3<f32>(lum)) * params2.x);
        let core = sat * (neon_glow * (0.85 + 0.8 * wisp));
        let sel_add = sat * (sel * (2.0 + 2.0 * wisp));
        return vec4<f32>(core + sel_add, 1.0);
    }

    // TRANSLUCENT EMPTY-POSITION MARKER → tiny clear grey glass: a faint body, a
    // cool-white haze (second medium), a bright Fresnel glass rim, and the
    // hover-selection swirl.
    let body = color.rgb * (neon_glow * (0.4 + 1.1 * wisp));
    let clear = vec3<f32>(0.85, 0.92, 1.0) * (haze * 0.4);
    let rim = mix(color.rgb, vec3<f32>(1.0), 0.4) * (rim_gain * fres);
    let sel_light = mix(color.rgb, vec3<f32>(1.0), 0.4) * (sel * (1.5 + 3.0 * wisp));
    let col = body + clear + rim + sel_light;
    let alpha = clamp(
        color.a * 0.3 + wisp * 0.4 + haze * 0.15 + fres * 0.7 + sel * 0.4,
        0.0,
        1.0,
    );
    return vec4<f32>(col, alpha);
}
