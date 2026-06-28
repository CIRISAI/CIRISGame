//! The four **Steward Signets** — glowing emblems in each steward's colour that
//! float outside the play area at the four horizontal cardinal directions
//! (E / W / N / S). With the up/down pole nebulae (`cube.rs`) they give all six
//! directions in space, so the player always knows their orientation.
//!
//! The signet of the steward whose turn it is burns brighter (×`boost`) than the
//! others, so the current player reads at a glance.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::palette;
use crate::render::BLOOM_LAYER;
use crate::BoardResource;
use ciris_game_engine_core::Steward;

/// Tunable signet parameters (driven by the tuning panel's Signet family).
#[derive(Resource)]
pub(crate) struct SignetSettings {
    /// Base emissive brightness of a non-active signet.
    pub bright: f32,
    /// Signet radius.
    pub size: f32,
    /// Distance from the board centre along its cardinal axis.
    pub dist: f32,
    /// Multiplier applied to the current steward's signet.
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

/// Unit direction each steward's signet floats along (E / N / W / S).
const DIRS: [Vec3; 4] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, -1.0),
];

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<SignetSettings>()
        .add_systems(Startup, spawn_signets)
        .add_systems(Update, update_signets);
}

fn spawn_signets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Unit sphere, scaled live by the size knob.
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    for slot in 0..4usize {
        let steward = Steward::ALL[slot];
        let c = palette::STEWARD_LINEAR[steward.slot() as usize].to_linear();
        // A glowing emissive emblem (HDR → blooms). `update_signets` drives the
        // emissive each frame (current steward burns brighter).
        let mat = mats.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(c.red * 4.0, c.green * 4.0, c.blue * 4.0),
            ..default()
        });
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat),
            Transform::default(),
            RenderLayers::from_layers(&[0, BLOOM_LAYER]),
            Signet(slot),
        ));
    }
}

fn update_signets(
    board: Res<BoardResource>,
    cfg: Res<SignetSettings>,
    mut q: Query<(&Signet, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let current = board.0.current_steward().slot() as usize;
    for (signet, mut tf, mat) in &mut q {
        tf.translation = DIRS[signet.0] * cfg.dist;
        tf.scale = Vec3::splat(cfg.size);
        let steward = Steward::ALL[signet.0];
        let c = palette::STEWARD_LINEAR[steward.slot() as usize].to_linear();
        let gain = if signet.0 == current {
            cfg.bright * cfg.boost
        } else {
            cfg.bright
        };
        if let Some(mut m) = mats.get_mut(&mat.0) {
            m.emissive = LinearRgba::rgb(c.red * gain, c.green * gain, c.blue * gain);
        }
    }
}
