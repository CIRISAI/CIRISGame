//! The DESIGN_BRIEF §3.8 horizon dome — a large inverted sphere carrying a
//! vertical Bone → Linen → Ochre gradient as vertex colours. It surrounds the
//! lattice so the glass shells have something warm to refract and reflect,
//! and gives the "warm-clay glass lab on a Bone-cream desk" backdrop (§0).
//!
//! The full §3.8 target is a procedural-HDR cubemap fed to `Skybox` for IBL;
//! that is deferred. TODO §3.8 IBL: bake the gradient to a cubemap + env map.

use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::palette;

/// Dome radius as a multiple of the board half-span; `scale = N/5`.
const DOME_RADIUS: f32 = 40.0;

/// Spawn the gradient horizon dome around the board.
pub fn spawn_dome(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    scale: f32,
) {
    let radius = DOME_RADIUS * scale;
    let mut mesh = Sphere::new(radius).mesh().uv(48, 32);
    apply_gradient(&mut mesh, radius);

    // Unlit so the gradient is the literal sky colour; `cull_mode = Front` keeps
    // only the inward-facing hull since the camera lives inside the dome.
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        cull_mode: Some(Face::Front),
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::default(),
    ));
}

/// Paint per-vertex colours from height: Ochre at the lower band, Linen across
/// the middle, Bone overhead (DESIGN_BRIEF §3.8). Colours are linear because
/// Bevy multiplies `ATTRIBUTE_COLOR` into `base_color` in linear space.
fn apply_gradient(mesh: &mut Mesh, radius: f32) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };

    let bottom = palette::OCHRE_LINEAR.to_linear();
    let mid = palette::LINEN_LINEAR.to_linear();
    let top = palette::BONE_LINEAR.to_linear();

    let colors: Vec<[f32; 4]> = positions
        .iter()
        .map(|p| {
            let t = (p[1] / radius * 0.5 + 0.5).clamp(0.0, 1.0);
            let c = if t < 0.5 {
                lerp(bottom, mid, t * 2.0)
            } else {
                lerp(mid, top, (t - 0.5) * 2.0)
            };
            [c.red, c.green, c.blue, 1.0]
        })
        .collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}

/// Component-wise linear interpolation between two linear colours.
fn lerp(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba::rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}
