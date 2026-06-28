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
// x = gas saturation (live-tuned); y = prism amount; zw spare.
@group(3) @binding(3) var<uniform> params2: vec4<f32>;
// x = IOR; y = refraction strength; z = cube half-size; w = edge glow gain.
@group(3) @binding(4) var<uniform> glass: vec4<f32>;

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

// The surrounding world — sampled procedurally along a ray DIRECTION, NEVER the
// scene, so other stones / tubes / markers never show through the glass. It's a
// deep-space starfield (we float in space): mostly black with sparse bright
// stars, so the glass refracts/reflects stars around its clear edge. Pure math,
// no texture, WebGL2-safe. `gain` scales star brightness (the "Reflect" knob).
fn star_env(d: vec3<f32>, gain: f32) -> vec3<f32> {
    // Starfield removed — the glass refracts/reflects the up/down POLE NEBULAE
    // instead, so the clear edge picks up the orientation colours (cool zenith,
    // warm nadir) against black rather than reading flat black.
    let nd = normalize(d);
    let up = smoothstep(0.35, 1.0, nd.y);
    let down = smoothstep(0.35, 1.0, -nd.y);
    let cool = vec3<f32>(0.12, 0.28, 0.85);
    let warm = vec3<f32>(0.95, 0.42, 0.16);
    return (cool * up + warm * down) * gain;
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
    // Selection: ONLY the exact cell under the cursor lights (tight falloff so
    // neighbours don't glow — the spikes show the connections instead).
    let sel = hover.w * exp(-distance(in.world_position.xyz, hover.xyz) * 6.0);

    if (color.a >= 0.999) {
        // CLEAR-GLASS MARBLE / TUBE WITH GAS INSIDE — opaque, so other stones /
        // tubes / markers never show through it. What you DO see "through" the
        // glass is the starfield only (star_env, pure math), refracted/reflected,
        // plus the steward's own swirling neon gas. Push SATURATION (not
        // brightness) so the muted pigment reads neon without blooming white.
        let lum = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        let sat = max(vec3<f32>(0.0), vec3<f32>(lum) + (color.rgb - vec3<f32>(lum)) * params2.x);
        let gas_light = sat * (neon_glow * (0.6 + 0.9 * wisp));

        let ior = glass.x;
        let edge = glass.w;
        let incident = -view_dir; // eye → surface
        let refl_dir = reflect(incident, nrm);
        let star_refl = star_env(refl_dir, edge);

        // `facing` is the radial coordinate of the surface: 1 dead-centre, 0 at
        // the silhouette. Glass is "thicker" at grazing angles → reflects more.
        let facing = clamp(dot(nrm, view_dir), 0.0, 1.0);
        let fres = pow(1.0 - facing, 3.0);

        // SELECTION / POSITION CUE: a strong bright glint that flares wherever the
        // cursor's emitted light reaches — "you are selecting here, it touches
        // these". Same formula on every surface type so it reads consistently,
        // brightest at the grazing rim where glass catches light.
        let sel_glint = mix(sat, vec3<f32>(1.0), 0.6) * (sel * (3.0 + 5.0 * fres));

        // X-cube dichroic prism rides the grazing rim when Prism is up.
        let prismatic = (nrm * nrm) * (neon_glow * 1.6 * fres * params2.y);

        // Spheres AND tubes render identically — the same gorgeous clear-glass
        // look: a clear glass edge that refracts + reflects the starfield (never
        // other game objects), wrapping a central neon gas core sized by glass.y.
        let refr_dir = refract(incident, nrm, 1.0 / ior);
        let star_refr = star_env(refr_dir, edge);
        let glass_view = mix(star_refr, star_refl, fres);
        let t_lo = clamp(1.0 - glass.y, 0.0, 0.95);
        let core_mask = smoothstep(t_lo, min(t_lo + 0.4, 0.98), facing);
        let gas_core = gas_light * core_mask;
        return vec4<f32>(glass_view + gas_core + prismatic + sel_glint, 1.0);
    }

    // TRANSLUCENT EMPTY-POSITION MARKER → tiny clear grey glass: a faint body, a
    // cool-white haze (second medium), a bright Fresnel glass rim, and the
    // hover-selection swirl.
    let body = color.rgb * (neon_glow * (0.4 + 1.1 * wisp));
    let clear = vec3<f32>(0.85, 0.92, 1.0) * (haze * 0.4);
    let rim = mix(color.rgb, vec3<f32>(1.0), 0.4) * (rim_gain * fres);
    // SELECTION / POSITION CUE — same strong glint as the marbles/tubes, so the
    // cursor's reach reads consistently across every surface type.
    let sel_glint = mix(color.rgb, vec3<f32>(1.0), 0.6) * (sel * (3.0 + 5.0 * fres));
    let col = body + clear + rim + sel_glint;
    let alpha = clamp(
        color.a * 0.3 + wisp * 0.4 + haze * 0.15 + fres * 0.7 + sel * 0.8,
        0.0,
        1.0,
    );
    return vec4<f32>(col, alpha);
}
