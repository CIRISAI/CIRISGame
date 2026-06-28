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
    let intensity = params.z + (1.0 - params.z) * n;
    let col = tint.rgb * (params.w * intensity);
    let alpha = tint.a * intensity;
    return vec4<f32>(col, alpha);
}
