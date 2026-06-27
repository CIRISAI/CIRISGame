//! Rendered presentation of the lattice (DESIGN_BRIEF §2–§3) and the
//! GameState → ECS sync. Feature-gated behind `render`; never compiled into the
//! headless build.
//!
//! The board is drawn from one persistent entity table built once at startup:
//! every cell owns a *frame* entity (glass shell or faint ghost marker), a
//! *core* entity (the emissive steward sphere, hidden unless the cell is live),
//! and a *ring* entity (Kaolin's Ink outline, hidden unless that cell is a live
//! Kaolin core). [`sync_board`] rewrites those entities' mesh/material/visibility
//! from [`BoardResource`] whenever [`BoardDirty`] is set, so the screensaver
//! driver (`screensaver.rs`) only has to flip a flag after each move.
//!
//! Material/lighting/environment construction lives in sibling modules
//! ([`crate::materials`], [`crate::lighting`], [`crate::environment`]); this
//! file owns geometry sizing, the entity table, and the sync.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode};
use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

use crate::{environment, lighting, materials, palette, screensaver};
use crate::{seed_from_counter, BoardResource};
use ciris_game_engine_core::{CellState, Coord, GameState, Steward, DEFAULT_BOARD_N};

/// Glass shell radius (DESIGN_BRIEF §3.1).
const SHELL_RADIUS: f32 = 0.42;
/// Inner core radius (DESIGN_BRIEF §3.1).
const CORE_RADIUS: f32 = 0.25;
/// Faint ghost-cell marker radius (placeholder for the §3.5 wireframe).
const GHOST_RADIUS: f32 = 0.09;
/// Bloom-layer index for emissive cores (DESIGN_BRIEF §2.3 / §3.3).
const BLOOM_LAYER: usize = 1;

/// Per-steward Gray-Scott seed PNGs (DESIGN_BRIEF §4.2), in steward-slot order.
const GS_SEED_PATHS: [&str; 4] = [
    "textures/gs-seed-sienna.png",
    "textures/gs-seed-lapis.png",
    "textures/gs-seed-verdigris.png",
    "textures/gs-seed-kaolin.png",
];

/// Shared mesh + material handles, built once at startup and cloned per cell.
#[derive(Resource)]
struct RenderAssets {
    shell_mesh: Handle<Mesh>,
    ghost_mesh: Handle<Mesh>,
    core_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    glass_mat: Handle<StandardMaterial>,
    tempdead_mat: Handle<StandardMaterial>,
    permadead_mat: Handle<StandardMaterial>,
    ghost_mat: Handle<StandardMaterial>,
    ring_mat: Handle<StandardMaterial>,
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
    /// Kaolin's Ink outline (hidden unless the cell is a live Kaolin core).
    ring: Vec<Entity>,
}

/// Handles to the Gray-Scott seed images, baked to pigment masks once loaded.
#[derive(Resource)]
struct GsPatterns {
    handles: [Handle<Image>; 4],
    /// True once every seed PNG has loaded and been baked in place.
    ready: bool,
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
        .add_systems(
            Update,
            (bake_gs_patterns, (screensaver::drive, sync_board).chain()),
        )
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
    asset_server: Res<AssetServer>,
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
            intensity: 0.18,
            composite_mode: BloomCompositeMode::EnergyConserving,
            ..default()
        },
        PanOrbitCamera {
            radius: Some(1.8 * n as f32),
            ..default()
        },
    ));

    // ── lighting rig + horizon dome (DESIGN_BRIEF §2.2 / §3.8) ───────────
    lighting::spawn_rig(&mut commands, scale);
    environment::spawn_dome(&mut commands, &mut meshes, &mut materials, scale);

    // ── Gray-Scott seed textures (DESIGN_BRIEF §4.2) ────────────────────
    // Loaded async; `bake_gs_patterns` converts each to a pigment mask in place
    // once it arrives. The core materials reference the handles immediately.
    let gs_handles: [Handle<Image>; 4] =
        std::array::from_fn(|slot| asset_server.load(GS_SEED_PATHS[slot]));

    // ── shared meshes + materials (DESIGN_BRIEF §3.1/§3.2/§3.3/§3.6) ─────
    let assets = RenderAssets {
        shell_mesh: meshes.add(Sphere::new(SHELL_RADIUS).mesh().ico(4).unwrap()),
        ghost_mesh: meshes.add(Sphere::new(GHOST_RADIUS).mesh().ico(2).unwrap()),
        // UV sphere for the core so the Gray-Scott pattern maps cleanly.
        core_mesh: meshes.add(Sphere::new(CORE_RADIUS).mesh().uv(48, 32)),
        ring_mesh: meshes.add(
            Sphere::new(CORE_RADIUS * materials::KAOLIN_RING_SCALE)
                .mesh()
                .ico(3)
                .unwrap(),
        ),
        glass_mat: materials.add(materials::glass()),
        tempdead_mat: materials.add(materials::tempdead()),
        permadead_mat: materials.add(materials::permadead()),
        ghost_mat: materials.add(materials::ghost()),
        ring_mat: materials.add(materials::kaolin_ring()),
        core_mats: [
            materials.add(materials::core(
                Steward::Sienna,
                Some(gs_handles[0].clone()),
            )),
            materials.add(materials::core(Steward::Lapis, Some(gs_handles[1].clone()))),
            materials.add(materials::core(
                Steward::Verdigris,
                Some(gs_handles[2].clone()),
            )),
            materials.add(materials::core(
                Steward::Kaolin,
                Some(gs_handles[3].clone()),
            )),
        ],
    };

    // ── one persistent frame + core + ring entity per cell ──────────────
    let count = board.0.board.len();
    let mut frame = Vec::with_capacity(count);
    let mut core = Vec::with_capacity(count);
    let mut ring = Vec::with_capacity(count);
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
        // Kaolin's Ink rim (§3.3). Layer 0 only so the outline never blooms;
        // shown by sync_board only for live Kaolin cells.
        ring.push(
            commands
                .spawn((
                    Mesh3d(assets.ring_mesh.clone()),
                    MeshMaterial3d(assets.ring_mat.clone()),
                    Transform::from_translation(pos),
                    Visibility::Hidden,
                ))
                .id(),
        );
    }

    commands.insert_resource(assets);
    commands.insert_resource(CellEntities { frame, core, ring });
    commands.insert_resource(GsPatterns {
        handles: gs_handles,
        ready: false,
    });
}

/// Bake each Gray-Scott seed image into a pigment mask once it has loaded
/// (DESIGN_BRIEF §4.2). Runs every frame until all four are ready, then idles.
fn bake_gs_patterns(mut gs: ResMut<GsPatterns>, mut images: ResMut<Assets<Image>>) {
    if gs.ready {
        return;
    }
    // Wait until every seed PNG has finished loading before baking any of them,
    // so the cores never flash the raw red/green field.
    if gs.handles.iter().any(|h| images.get(h).is_none()) {
        return;
    }
    for handle in &gs.handles {
        if let Some(mut image) = images.get_mut(handle) {
            materials::bake_pigment_mask(&mut image);
        }
    }
    gs.ready = true;
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
        let ring = cells.ring[idx];
        // Kaolin's rim only shows for a live Kaolin core; default it off.
        let mut ring_visible = false;
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
                ring_visible = steward == Steward::Kaolin;
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
        commands.entity(ring).insert(if ring_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
    dirty.0 = false;
}
