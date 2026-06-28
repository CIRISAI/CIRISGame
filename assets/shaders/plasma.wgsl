// Flowing-plasma ghost wireframe (DESIGN_BRIEF §3.5, reimagined).
//
// A fragment shader for the empty-cell LineList cage: layered sines in world
// space, scrolling over time, make the lines shimmer like gentle plasma —
// translucent and ethereal. WebGL2-safe: no compute, no texture sampling, no
// loops, vec4-aligned uniforms, alpha-blended.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

// rgb = plasma tint (linear); a = base opacity.
@group(3) @binding(0) var<uniform> tint: vec4<f32>;
// x = flow speed; y = spatial freq; z = floor brightness; w = glow gain.
@group(3) @binding(1) var<uniform> params: vec4<f32>;
// xyz = cursor focus point (world); w = strength [0,1]. 0 = resting cage.
@group(3) @binding(2) var<uniform> hover: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time * params.x;
    let f = params.y;
    let p = in.world_position.xyz;

    // Layered travelling waves → a soft flowing field in [0, 1].
    let w = sin(p.x * f + t)
        + sin(p.y * f * 0.9 - t * 0.8)
        + sin(p.z * f * 1.1 + t * 0.6)
        + sin((p.x + p.y + p.z) * f * 0.6 + t * 0.4);
    let n = clamp(w * 0.125 + 0.5, 0.0, 1.0);

    // Floor brightness keeps the cage always faintly visible; peaks glow.
    var intensity = params.z + (1.0 - params.z) * n;

    // Cursor attention: the plasma rushes IN toward the focus point. `prox`
    // peaks at the hovered cell and falls off with distance; an inward-moving
    // pulse (phase advances with −distance) sells the "rushing in" motion.
    let hd = distance(p, hover.xyz);
    let prox = exp(-hd * 1.7);
    let rush = 0.55 + 0.45 * sin(hd * (f * 2.2) - globals.time * (params.x * 7.0));
    let attn = hover.w * prox * rush;
    intensity = intensity + attn * 2.6;

    // HDR (>1) at peaks + near the cursor so Bloom turns the strands ethereal.
    let col = tint.rgb * (params.w * intensity);
    let alpha = clamp(tint.a * intensity, 0.0, 1.0);
    return vec4<f32>(col, alpha);
}
