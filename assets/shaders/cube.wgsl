// Play-area cube (DBS tournament arena): five faces one colour, the sixth (+Y, the
// "top") an accent colour, at a tunable opacity, with a tight bright silhouette
// edge. Double-sided + alpha-blended so it reads as one clear box.
//
// WebGL2-safe: fragment-only, vec4 uniforms, alpha-blended, no derivatives.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}

// rgb = the five faces' colour (linear); a = cube opacity.
@group(3) @binding(0) var<uniform> color: vec4<f32>;
// rgb = the sixth (+Y) face's accent colour; a spare.
@group(3) @binding(1) var<uniform> accent: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let nrm = normalize(in.world_normal);
    // The +Y face gets the accent colour; the other five share `color`.
    var face = color.rgb;
    if (nrm.y > 0.5) {
        face = accent.rgb;
    }

    // Tight silhouette edge so the box reads as glass without washing the faces.
    let vd = normalize(view.world_position.xyz - in.world_position.xyz);
    let fres = pow(1.0 - clamp(abs(dot(nrm, vd)), 0.0, 1.0), 5.0);

    let col = face * (0.7 + 1.3 * fres);
    let alpha = clamp(color.a + fres * 0.5, 0.0, 1.0);
    return vec4<f32>(col, alpha);
}
