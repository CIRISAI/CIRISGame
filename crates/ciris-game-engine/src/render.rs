//! Rendered presentation of the lattice (DESIGN_BRIEF §2–§3) and the
//! GameState → ECS sync. Feature-gated behind `render`; never compiled into the
//! headless build.
//!
//! The board is drawn from one persistent entity table built once at startup:
//! every cell owns a *frame* entity (glass shell or faint ghost marker) and a
//! *core* entity (the emissive steward sphere, hidden unless the cell is live).
//! [`sync_board`] rewrites those entities' mesh/material/visibility from
//! [`BoardResource`] whenever [`BoardDirty`] is set, so the screensaver driver
//! (`screensaver.rs`) only has to flip a flag after each move.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::GlobalAmbientLight;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode};
use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

use crate::screensaver;
use crate::{palette, seed_from_counter, BoardResource};
use ciris_game_engine_core::{CellState, Coord, GameState, Steward, DEFAULT_BOARD_N};

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

/// Shared mesh + material handles, built once at startup and cloned per cell.
#[derive(Resource)]
struct RenderAssets {
    shell_mesh: Handle<Mesh>,
    ghost_mesh: Handle<Mesh>,
    core_mesh: Handle<Mesh>,
    glass_mat: Handle<StandardMaterial>,
    tempdead_mat: Handle<StandardMaterial>,
    permadead_mat: Handle<StandardMaterial>,
    ghost_mat: Handle<StandardMaterial>,
    /// Emissive core material per steward slot (0..=3).
    core_mats: [Handle<StandardMaterial>; 4],
}

/// Per-cell entity table, indexed by linear board index.
#[derive(Resource)]
struct CellEntities {
    /// The shell-or-ghost frame entity for each cell.
    frame: Vec<Entity>,
    /// The emissive core entity for each cell (hidden unless the cell is live).
    core: Vec<Entity>,
}

/// Set whenever the board changes; [`sync_board`] consumes it. Starts `true` so
/// the initial all-empty board is painted on the first frame.
#[derive(Resource)]
pub struct BoardDirty(pub bool);

/// Build the App and run it (windowed / wasm).
pub fn run_app() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CIRISGame".into(),
                // wasm: bind to the <canvas id="bevy"> in the page shim and let
                // it track the parent element (DESIGN_BRIEF §6.5).
                canvas: Some("#bevy".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PanOrbitCameraPlugin)
        .insert_resource(ClearColor(palette::BONE_SRGB))
        .insert_resource(BoardResource(GameState::new(
            DEFAULT_BOARD_N,
            seed_from_counter(0),
        )))
        .insert_resource(BoardDirty(true))
        .insert_resource(screensaver::ScreensaverState::new())
        .insert_resource(screensaver::AiRng::new(0))
        .add_systems(Startup, setup)
        .add_systems(Update, (screensaver::drive, sync_board).chain())
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
    board: Res<BoardResource>,
) {
    let n = board.0.board.n;
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

    // ── shared meshes + materials (DESIGN_BRIEF §3.1/§3.2/§3.6) ──────────
    let assets = RenderAssets {
        shell_mesh: meshes.add(Sphere::new(SHELL_RADIUS).mesh().ico(4).unwrap()),
        ghost_mesh: meshes.add(Sphere::new(GHOST_RADIUS).mesh().ico(2).unwrap()),
        core_mesh: meshes.add(Sphere::new(CORE_RADIUS).mesh().ico(3).unwrap()),
        glass_mat: materials.add(glass_material()),
        tempdead_mat: materials.add(tempdead_material()),
        permadead_mat: materials.add(permadead_material()),
        ghost_mat: materials.add(ghost_material()),
        core_mats: [
            materials.add(core_material(Steward::Sienna)),
            materials.add(core_material(Steward::Lapis)),
            materials.add(core_material(Steward::Verdigris)),
            materials.add(core_material(Steward::Kaolin)),
        ],
    };

    // ── one persistent frame + core entity per cell ─────────────────────
    let count = board.0.board.len();
    let mut frame = Vec::with_capacity(count);
    let mut core = Vec::with_capacity(count);
    for idx in 0..count {
        let pos = cell_world_pos(board.0.board.coord(idx), n);
        // Frame starts as a ghost marker; sync_board paints the real state.
        frame.push(
            commands
                .spawn((
                    Mesh3d(assets.ghost_mesh.clone()),
                    MeshMaterial3d(assets.ghost_mat.clone()),
                    Transform::from_translation(pos),
                ))
                .id(),
        );
        // Cores on layers [0, 1]: PBR-shaded on layer 0, glow on layer 1
        // (DESIGN_BRIEF §3.3). Hidden until the cell goes live.
        core.push(
            commands
                .spawn((
                    Mesh3d(assets.core_mesh.clone()),
                    MeshMaterial3d(assets.core_mats[0].clone()),
                    Transform::from_translation(pos),
                    Visibility::Hidden,
                    RenderLayers::from_layers(&[0, BLOOM_LAYER]),
                ))
                .id(),
        );
        // TODO §3.3: Kaolin's mandatory 2px Ink rim ring.
    }

    commands.insert_resource(assets);
    commands.insert_resource(CellEntities { frame, core });
}

/// Rewrite the per-cell entities from the live board, when something changed.
fn sync_board(
    mut dirty: ResMut<BoardDirty>,
    board: Res<BoardResource>,
    cells: Res<CellEntities>,
    assets: Res<RenderAssets>,
    mut commands: Commands,
) {
    if !dirty.0 {
        return;
    }
    let gs = &board.0;
    for idx in 0..gs.board.len() {
        let frame = cells.frame[idx];
        let core = cells.core[idx];
        match gs.board.get(idx) {
            // Empty → faint ghost marker, core hidden (DESIGN_BRIEF §3.5).
            CellState::Empty => {
                commands.entity(frame).insert((
                    Mesh3d(assets.ghost_mesh.clone()),
                    MeshMaterial3d(assets.ghost_mat.clone()),
                ));
                commands.entity(core).insert(Visibility::Hidden);
            }
            // Live → glass shell + emissive steward core (§3.2/§3.3).
            CellState::Live(steward) => {
                commands.entity(frame).insert((
                    Mesh3d(assets.shell_mesh.clone()),
                    MeshMaterial3d(assets.glass_mat.clone()),
                ));
                commands.entity(core).insert((
                    MeshMaterial3d(assets.core_mats[steward.slot() as usize].clone()),
                    Visibility::Visible,
                ));
            }
            // Temp-dead → darkened shell, core off (§3.6). TODO §3.6: black mist.
            CellState::TempDead(_) => {
                commands.entity(frame).insert((
                    Mesh3d(assets.shell_mesh.clone()),
                    MeshMaterial3d(assets.tempdead_mat.clone()),
                ));
                commands.entity(core).insert(Visibility::Hidden);
            }
            // Perma-dead → Verdigris-tinted shell, core off (§3.6). TODO §3.6:
            // green mist.
            CellState::PermaDead => {
                commands.entity(frame).insert((
                    Mesh3d(assets.shell_mesh.clone()),
                    MeshMaterial3d(assets.permadead_mat.clone()),
                ));
                commands.entity(core).insert(Visibility::Hidden);
            }
        }
    }
    dirty.0 = false;
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

/// Glass shell (DESIGN_BRIEF §3.2).
fn glass_material() -> StandardMaterial {
    StandardMaterial {
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
    }
}

/// Temp-dead shell: desaturated and dark, transmission killed (DESIGN_BRIEF §3.6).
fn tempdead_material() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB,
        perceptual_roughness: 0.9,
        reflectance: 0.2,
        ..default()
    }
}

/// Perma-dead shell: Verdigris-tinted neutral substrate (DESIGN_BRIEF §3.6).
fn permadead_material() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::STEWARD_VERDIGRIS_SRGB,
        specular_transmission: 0.3,
        ior: 1.50,
        thickness: 0.18,
        perceptual_roughness: 0.25,
        attenuation_color: palette::STEWARD_VERDIGRIS_LINEAR,
        attenuation_distance: 1.4,
        reflectance: 0.35,
        ..default()
    }
}

/// Faint ghost marker for empty cells (DESIGN_BRIEF §3.5 placeholder).
fn ghost_material() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB.with_alpha(0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
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
