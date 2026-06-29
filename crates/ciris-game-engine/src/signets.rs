//! The four **Steward Signets** — a distinct glass solid per steward, floating
//! outside the play area at the four horizontal cardinal directions (E/W/N/S).
//! Each is rendered in the steward's glass-gas material (`OrbMaterial`), so the
//! signets match the marbles' look. The shapes echo the topology dial:
//! Sienna = cube, Lapis = toroid, Verdigris = rhombus (octahedron),
//! Kaolin = Möbius strip. With the up/down pole nebulae they give the player all
//! six orientation directions; the signet of the steward to move burns brighter.

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::orb::{self, OrbMaterial};
use crate::render::BLOOM_LAYER;
use crate::BoardResource;
use ciris_game_engine_core::Steward;

/// Tunable signet parameters (driven by the tuning panel's Signet family).
#[derive(Resource)]
pub(crate) struct SignetSettings {
    /// Base gas-glow of a non-active signet.
    pub bright: f32,
    /// Signet scale.
    pub size: f32,
    /// Distance from the board centre along its cardinal axis.
    pub dist: f32,
    /// Glow multiplier applied to the current steward's signet.
    pub boost: f32,
}

impl Default for SignetSettings {
    fn default() -> Self {
        SignetSettings {
            bright: 0.5,
            size: 0.5,
            dist: 15.0,
            boost: 10.0,
        }
    }
}

/// Tags a signet with its steward slot.
#[derive(Component)]
struct Signet(usize);

/// The four signet materials, so the current steward's can be brightened.
#[derive(Resource)]
struct SignetMats([Handle<OrbMaterial>; 4]);

/// Unit direction each steward's signet floats along (E / N / W / S).
const DIRS: [Vec3; 4] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, -1.0),
];

/// Distinct idle-spin axis per signet so each 3D shape reads.
const SPIN_AXES: [Vec3; 4] = [
    Vec3::new(0.3, 1.0, 0.0),
    Vec3::new(0.0, 1.0, 0.4),
    Vec3::new(0.5, 1.0, 0.2),
    Vec3::new(0.2, 1.0, 0.6),
];

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<SignetSettings>()
        .add_systems(Startup, spawn_signets)
        .add_systems(Update, update_signets);
}

fn spawn_signets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut orb_mats: ResMut<Assets<OrbMaterial>>,
) {
    // Sienna = cube, Lapis = toroid, Verdigris = rhombus, Kaolin = Möbius.
    let shapes = [
        meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        meshes.add(Torus {
            major_radius: 0.62,
            minor_radius: 0.26,
        }),
        meshes.add(rhombus()),
        meshes.add(mobius()),
    ];
    let mut mats: [Handle<OrbMaterial>; 4] = std::array::from_fn(|_| Handle::default());
    for slot in 0..4usize {
        let steward = Steward::ALL[slot];
        // Steward glass-gas material, tuned to read vividly at signet scale.
        let mut m = orb::material(steward);
        m.params2.x = 6.0; // gas saturation
        m.glass.y = 0.85; // bigger gas core fills the small solid
        let handle = orb_mats.add(m);
        mats[slot] = handle.clone();
        commands.spawn((
            Mesh3d(shapes[slot].clone()),
            MeshMaterial3d(handle),
            Transform::default(),
            RenderLayers::from_layers(&[0, BLOOM_LAYER]),
            Signet(slot),
        ));
    }
    commands.insert_resource(SignetMats(mats));
}

fn update_signets(
    time: Res<Time>,
    board: Res<BoardResource>,
    cfg: Res<SignetSettings>,
    mats: Res<SignetMats>,
    mut orb_mats: ResMut<Assets<OrbMaterial>>,
    mut q: Query<(&Signet, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (signet, mut tf) in &mut q {
        tf.translation = DIRS[signet.0] * cfg.dist;
        tf.scale = Vec3::splat(cfg.size);
        // Slow idle spin so the solid's form reads at distance.
        tf.rotation = Quat::from_axis_angle(SPIN_AXES[signet.0].normalize(), t * 0.25);
    }
    let current = board.0.current_steward().slot() as usize;
    for (slot, h) in mats.0.iter().enumerate() {
        if let Some(mut m) = orb_mats.get_mut(h) {
            let boost = if slot == current { cfg.boost } else { 1.0 };
            m.params.z = cfg.bright * boost * 2.2;
        }
    }
}

/// A real rhombus-shaped extrusion.
fn rhombus() -> Mesh {
    // Extrude a 2D rhombus into 3D. The Extrusion places the rhombus on the XY plane
    // and extrudes it along the Z axis by the given depth. We will use a scale
    // matching roughly the other shapes (like the toroid with major 0.62).
    // `Rhombus::new(half_x, half_y)`
    Mesh::from(Extrusion::new(Rhombus::new(0.6, 0.8), 0.6))
}

/// A Möbius band (the Kaolin signet), built double-sided (front + back tris) so
/// it renders from both faces without a double-sided material.
fn mobius() -> Mesh {
    const US: usize = 96;
    const VS: usize = 4;
    let r = 0.62;
    let halfw = 0.30;
    let pt = |u: f32, v: f32| -> Vec3 {
        let hu = u * 0.5;
        Vec3::new(
            (r + v * hu.cos()) * u.cos(),
            v * hu.sin(),
            (r + v * hu.cos()) * u.sin(),
        )
    };
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    for i in 0..=US {
        let u = i as f32 / US as f32 * TAU;
        for j in 0..=VS {
            let v = (j as f32 / VS as f32 - 0.5) * 2.0 * halfw;
            let p = pt(u, v);
            let du = pt(u + 0.01, v) - pt(u - 0.01, v);
            let dv = pt(u, v + 0.01) - pt(u, v - 0.01);
            let n = du.cross(dv).normalize_or_zero();
            pos.push([p.x, p.y, p.z]);
            nrm.push([n.x, n.y, n.z]);
            uv.push([i as f32 / US as f32, j as f32 / VS as f32]);
        }
    }
    let stride = (VS + 1) as u32;
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..US as u32 {
        for j in 0..VS as u32 {
            let a = i * stride + j;
            let b = (i + 1) * stride + j;
            let c = i * stride + j + 1;
            let d = (i + 1) * stride + j + 1;
            idx.extend([a, b, c, c, b, d]); // front
            idx.extend([a, c, b, c, d, b]); // back (double-sided)
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}
