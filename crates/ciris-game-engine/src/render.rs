//! Rendered presentation of the lattice (DESIGN_BRIEF §2–§3). Feature-gated
//! behind `render`; never compiled into the headless build.
//!
//! First cut: for every cell a glass shell (§3.2), an emissive steward core for
//! occupied cells (§3.3), and a faint ghost marker for empty cells (§3.5). The
//! §2.2 lighting rig (scaled by N/5) and a panorbit camera with Bloom + AgX
//! tonemapping (§2.3) frame the board.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::GlobalAmbientLight;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode};
use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

use crate::palette;
use ciris_game_engine_core::{Board, CellState, Coord, Steward, DEFAULT_BOARD_N};

/// Glass shell radius (DESIGN_BRIEF §3.1).
const SHELL_RADIUS: f32 = 0.42;
/// Inner core radius (DESIGN_BRIEF §3.1).
const CORE_RADIUS: f32 = 0.25;
/// Faint ghost-cell marker radius (placeholder for the §3.5 wireframe).
const GHOST_RADIUS: f32 = 0.09;
/// Default core emissive intensity (DESIGN_BRIEF §3.3, range [0.4, 1.8]).
const CORE_EMISSIVE: f32 = 0.6;
/// Bloom-layer index for emissive cores (DESIGN_BRIEF §2.3 / §3.3).
const BLOOM_LAYER: usize = 1;

/// Build the App and run it (windowed).
pub fn run_app() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CIRISGame".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PanOrbitCameraPlugin)
        .insert_resource(ClearColor(palette::BONE_SRGB))
        .add_systems(Startup, setup)
        .run();
}

/// World-space center of lattice cell `(i, j, k)` for an `n³` board:
/// `world = coord − (n−1)/2` per axis (DESIGN_BRIEF §3.1).
fn cell_world_pos(c: Coord, n: u8) -> Vec3 {
    let half = (n as f32 - 1.0) / 2.0;
    Vec3::new(c.i as f32 - half, c.j as f32 - half, c.k as f32 - half)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let n = DEFAULT_BOARD_N;
    // N/5 lighting-rig + camera scale factor (DESIGN_BRIEF §2.2 / §4.4).
    let scale = n as f32 / 5.0;

    // ── camera (DESIGN_BRIEF §2.3 / §4.4) ───────────────────────────────
    // Single-camera first cut: Hdr + Bloom + AgX on the panorbit rig. Cores sit
    // on RenderLayers [0, 1] so the §2.3 two-camera selective-bloom split can be
    // layered in later without moving geometry.
    // TODO §2.3: split into Camera A (layer 0, no bloom) + Camera B (layer 1,
    // bloom) over one render target for selective core glow.
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::AgX,
        Bloom {
            intensity: 0.15,
            composite_mode: BloomCompositeMode::EnergyConserving,
            ..default()
        },
        PanOrbitCamera {
            radius: Some(1.8 * n as f32),
            ..default()
        },
    ));

    // ── lighting rig, N = 5 baseline scaled by N/5 (DESIGN_BRIEF §2.2) ───
    // TODO §2.2: calibrate illuminance + add the hemispheric sky gradient and
    // the Skybox/IBL horizon dome (§3.8). Directional stand-ins for now.
    spawn_key_light(
        &mut commands,
        scale,
        Vec3::new(6.67, 9.17, 5.00),
        Color::srgb_u8(0xFF, 0xE5, 0xCC),
        10_000.0 * 1.6,
    );
    spawn_key_light(
        &mut commands,
        scale,
        Vec3::new(-5.83, 2.50, 4.17),
        Color::srgb_u8(0xDC, 0xE5, 0xEF),
        10_000.0 * 0.55,
    );
    spawn_key_light(
        &mut commands,
        scale,
        Vec3::new(0.83, 3.67, -7.50),
        Color::srgb_u8(0xFF, 0xD6, 0xA8),
        10_000.0 * 1.2,
    );
    commands.insert_resource(GlobalAmbientLight {
        color: palette::BOROSILICATE_SRGB,
        brightness: 350.0,
        ..default()
    });

    // ── shared meshes + glass-shell material (DESIGN_BRIEF §3.1/§3.2) ────
    let shell_mesh = meshes.add(Sphere::new(SHELL_RADIUS).mesh().ico(4).unwrap());
    let core_mesh = meshes.add(Sphere::new(CORE_RADIUS).mesh().ico(3).unwrap());
    let ghost_mesh = meshes.add(Sphere::new(GHOST_RADIUS).mesh().ico(2).unwrap());

    let shell_material = materials.add(StandardMaterial {
        base_color: palette::BOROSILICATE_SRGB,
        specular_transmission: 1.0,
        ior: 1.50,
        thickness: 0.18,
        perceptual_roughness: 0.04,
        attenuation_color: palette::BOROSILICATE_LINEAR,
        attenuation_distance: 2.2,
        reflectance: 0.5,
        // TODO §3.2: ExtendedMaterial<StandardMaterial, RimMaterial> Fresnel rim.
        ..default()
    });

    let ghost_material = materials.add(StandardMaterial {
        base_color: palette::SLATE_SRGB.with_alpha(0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // ── the board ───────────────────────────────────────────────────────
    // Placeholder demo state: one stone per steward so all four core pigments
    // read in this first cut. The rules system (BACKLOG #6) will own real state.
    // TODO #6: drive cells from the BoardResource mutated by the engine.
    let mut board = Board::new(n);
    let demo: [(Coord, Steward); 4] = [
        (Coord::new(2, 2, 2), Steward::Sienna),
        (Coord::new(1, 2, 2), Steward::Lapis),
        (Coord::new(2, 1, 2), Steward::Verdigris),
        (Coord::new(2, 2, 1), Steward::Kaolin),
    ];
    for (c, s) in demo {
        if let Some(idx) = board.index(c) {
            board.set(idx, CellState::Live(s));
        }
    }

    let mut core_materials: [Option<Handle<StandardMaterial>>; 4] = [None, None, None, None];

    for idx in 0..board.len() {
        let c = board.coord(idx);
        let pos = cell_world_pos(c, n);

        // Every cell gets a glass shell.
        commands.spawn((
            Mesh3d(shell_mesh.clone()),
            MeshMaterial3d(shell_material.clone()),
            Transform::from_translation(pos),
        ));

        match board.get(idx) {
            CellState::Live(steward) => {
                let slot = steward.slot() as usize;
                let mat = core_materials[slot]
                    .get_or_insert_with(|| materials.add(core_material(steward)))
                    .clone();
                // Cores on layers [0, 1]: PBR-shaded on layer 0, glow on layer 1
                // (DESIGN_BRIEF §3.3).
                commands.spawn((
                    Mesh3d(core_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(pos),
                    RenderLayers::from_layers(&[0, BLOOM_LAYER]),
                ));
                // TODO §3.3: Kaolin's mandatory 2px Ink rim ring.
            }
            CellState::Empty => {
                // TODO §3.5: replace with a bevy_polyline rhombic-dodecahedron
                // wireframe in Slate at distance-faded alpha.
                commands.spawn((
                    Mesh3d(ghost_mesh.clone()),
                    MeshMaterial3d(ghost_material.clone()),
                    Transform::from_translation(pos),
                ));
            }
            // TODO §3.6: temp-dead (black) and perma-dead (green) mist.
            CellState::TempDead(_) | CellState::PermaDead => {}
        }
    }
}

/// A directional light aimed at the board center from `pos`, scaled by N/5.
fn spawn_key_light(commands: &mut Commands, scale: f32, pos: Vec3, color: Color, illuminance: f32) {
    commands.spawn((
        DirectionalLight {
            color,
            illuminance,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(pos * scale).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Emissive steward-core material (DESIGN_BRIEF §3.3).
fn core_material(steward: Steward) -> StandardMaterial {
    let slot = steward.slot() as usize;
    StandardMaterial {
        base_color: palette::STEWARD_SRGB[slot],
        emissive: palette::STEWARD_LINEAR[slot].to_linear() * CORE_EMISSIVE,
        perceptual_roughness: 0.35,
        // TODO §3.3: sample the per-mesh 96×96 Gray-Scott R-D texture.
        ..default()
    }
}
