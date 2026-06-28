//! Rendered presentation of the lattice (DESIGN_BRIEF §2–§3) and the
//! GameState → ECS sync. Feature-gated behind `render`; never compiled into the
//! headless build.
//!
//! The board is drawn from one persistent entity table built once at startup:
//! every cell owns a *frame* entity (glass shell or ghost wireframe), a
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
use bevy::camera::{Hdr, Viewport};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

use crate::orb::OrbMaterial;
use crate::state::AppScreen;
use crate::{
    attract, cube, effects, endgame, environment, fonts, hover, i18n, intro, lighting, materials,
    mist, navigation, orb, palette, pipe, plasma, screensaver, state, ui_theme, wizard,
};
use crate::{seed_from_counter, BoardResource};
use ciris_game_engine_core::{CellState, Coord, GameState, Steward, DEFAULT_BOARD_N};

/// Glass shell radius (DESIGN_BRIEF §3.1). `pub(crate)` so the effects layer can
/// size orbit radii and pipe spans against it.
pub(crate) const SHELL_RADIUS: f32 = 0.42;
/// Opaque steward-core radius (DESIGN_BRIEF §3.1/§3.3): a small bright neon core
/// suspended in a big clear-glass marble (0.25 vs the 0.42 shell), so the thick
/// glass lenses/refracts the core and the clear refractive rim clearly reads.
const CORE_RADIUS: f32 = 0.25;
/// Bloom-layer index for emissive cores (DESIGN_BRIEF §2.3 / §3.3). Shared with
/// the effects layer so motes glow on the same layer.
pub(crate) const BLOOM_LAYER: usize = 1;

/// Shared mesh + material handles, built once at startup and cloned per cell.
#[derive(Resource)]
struct RenderAssets {
    shell_mesh: Handle<Mesh>,
    /// Small sphere marking a lattice position where a sphere could be placed
    /// (empty cells) — a tiny clear grey glass orb. Replaces the plasma wireframe.
    dot_mesh: Handle<Mesh>,
    core_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    /// Thick clear glass shell that refracts the opaque steward core into a
    /// marble (DESIGN_BRIEF §3.2).
    glass_mat: Handle<StandardMaterial>,
    tempdead_mat: Handle<StandardMaterial>,
    permadead_mat: Handle<StandardMaterial>,
    ring_mat: Handle<StandardMaterial>,
    /// Tiny clear-grey glass orb for empty positions (DESIGN_BRIEF §3.5 reimagined).
    empty_orb: Handle<OrbMaterial>,
    /// Steward orb material per slot (0..=3) — the whole live sphere: thick clear
    /// glass + two swirling gasses, one surface (DESIGN_BRIEF §3.2/§3.3).
    core_orb: [Handle<OrbMaterial>; 4],
}

/// Per-cell entity table, indexed by linear board index.
#[derive(Resource)]
struct CellEntities {
    /// The glass-shell-or-ghost-wireframe frame entity for each cell.
    frame: Vec<Entity>,
    /// The emissive core entity for each cell (hidden unless the cell is live).
    core: Vec<Entity>,
    /// Kaolin's Ink outline (hidden unless the cell is a live Kaolin core).
    ring: Vec<Entity>,
}

/// Set whenever the board changes; [`sync_board`] consumes it. Starts `true` so
/// the initial all-empty board is painted on the first frame.
#[derive(Resource)]
pub struct BoardDirty(pub bool);

/// The board's cell states as of the last [`sync_board`] pass. Diffed against the
/// live board each [`BoardDirty`] to detect the §4.6 collapse / dispersal
/// transitions that drive the mist and cascade animations.
#[derive(Resource)]
pub(crate) struct PrevStates(pub Vec<CellState>);

/// Per-cell transition flags for the current move, written by [`sync_board`] and
/// read by the effect layer (core birth-in, pipe extrude). Reset every diff.
#[derive(Resource)]
pub(crate) struct Transitions {
    /// Cells that became `Live` this move (any → Live): §4.6 core fade-in + the
    /// new-pipe extrude.
    pub became_live: Vec<bool>,
}

/// Build the App and run it (windowed / wasm).
pub fn run_app() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
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
    // Layer-traversal fly-through layered on top of the panorbit rig (§4.8).
    .add_plugins(navigation::plugin)
    .add_plugins(mist::plugin)
    // Liquid-pigment pipes: the custom `PipeMaterial` + the camera-driven slosh.
    .add_plugins(pipe::plugin)
    .add_plugins(plasma::plugin)
    .add_plugins(orb::plugin)
    // DBS tournament grid-cube enclosure + dark/light mode selector.
    .add_plugins(cube::plugin)
    // Cursor-attention: hovered cell glows + plasma rushes inward (hover.rs).
    .add_plugins(hover::plugin)
    // Load the §5.1 UI faces so the front-of-house text actually renders.
    .add_plugins(fonts::plugin)
    // Front-of-house: the Intro → Setup → Playing state machine (`state.rs`),
    // the click-through intro (`intro.rs`), and the setup wizard (`wizard.rs`).
    // The screensaver below keeps advancing in every state.
    .add_plugins((
        state::plugin,
        attract::plugin,
        intro::plugin,
        wizard::plugin,
    ))
    .init_resource::<i18n::Localization>()
    .insert_resource(ClearColor(palette::INK_SRGB))
    .insert_resource(BoardResource(GameState::new(
        DEFAULT_BOARD_N,
        seed_from_counter(0),
    )))
    .insert_resource(BoardDirty(true))
    .insert_resource(screensaver::ScreensaverState::new())
    .insert_resource(screensaver::AiRng::new(0))
    .init_resource::<effects::CoreScale>()
    .init_resource::<endgame::Ending>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            // GameState → board entities → effect parameters → endgame, in
            // order; `clear_board_dirty` runs last so every consumer of
            // `BoardDirty` (sync_board, sync_effects) sees the same flag.
            (
                screensaver::drive,
                sync_board,
                effects::sync_effects,
                endgame::drive_endgame,
                clear_board_dirty,
            )
                .chain(),
            // Per-frame motion reads the parameters above (one-frame latency
            // on a fresh board is imperceptible at the screensaver cadence).
            effects::breathe_cores,
            effects::grow_pipes,
            mist::animate_mist,
            // Contain the 3D to the hero rect in Intro/Setup, full in Playing.
            update_camera_viewport,
            // Hover/press feedback for every front-of-house button.
            ui_theme::button_visuals,
        ),
    );

    // Dev screenshot capture is native-only (see `capture.rs`).
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(crate::capture::plugin);

    app.run();
}

/// Resize the 3D camera's viewport so the resting hero renders into a contained
/// sub-rectangle during Intro/Setup (the surrounding window is the Bone overlay
/// from [`ui_theme::hero_overlay`]) and fills the whole window in Playing. The
/// hero rectangle fractions are shared with the overlay so the framed Bone border
/// and the 3D viewport always coincide. Recomputed every frame from the window's
/// physical size, so it also tracks `WindowResized` and the initial layout.
fn update_camera_viewport(
    screen: Res<State<AppScreen>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut camera: Query<&mut Camera, With<Camera3d>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(mut camera) = camera.single_mut() else {
        return;
    };
    let size = window.physical_size();
    if size.x == 0 || size.y == 0 {
        return;
    }

    let want = ui_theme::hero_rect(*screen.get()).map(|[l, t, w, h]| {
        let sx = size.x as f32;
        let sy = size.y as f32;
        let pos = UVec2::new((l * sx) as u32, (t * sy) as u32);
        let dim = UVec2::new((w * sx).max(1.0) as u32, (h * sy).max(1.0) as u32);
        Viewport {
            physical_position: pos,
            physical_size: dim,
            depth: 0.0..1.0,
        }
    });

    // `Viewport` isn't `PartialEq`; compare the fields that matter so we don't
    // mark the camera changed every frame.
    let changed = match (&camera.viewport, &want) {
        (None, None) => false,
        (Some(a), Some(b)) => {
            a.physical_position != b.physical_position || a.physical_size != b.physical_size
        }
        _ => true,
    };
    if changed {
        camera.viewport = want;
    }
}

/// World-space center of lattice cell `(i, j, k)` for an `n³` board:
/// `world = coord − (n−1)/2` per axis (DESIGN_BRIEF §3.1). `pub(crate)` so the
/// effects layer can cache cell centres for orbits and pipes.
pub(crate) fn cell_world_pos(c: Coord, n: u8) -> Vec3 {
    let half = (n as f32 - 1.0) / 2.0;
    Vec3::new(c.i as f32 - half, c.j as f32 - half, c.k as f32 - half)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mist_materials: ResMut<Assets<mist::MistMaterial>>,
    mut orb_materials: ResMut<Assets<OrbMaterial>>,
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
        // Order 0: clears Bone (within its viewport) and draws the lattice. In
        // Intro/Setup `update_camera_viewport` shrinks this to the hero rect.
        Camera {
            order: 0,
            ..default()
        },
        Hdr,
        // HDR Rgba16Float can't be multisampled (esp. on the webgl2 build), and a
        // mix of HDR/non-HDR cameras renders magenta on Metal (bevy #6754) — so
        // BOTH cameras are Hdr + Msaa::Off, consistently, in every state.
        Msaa::Off,
        Tonemapping::AgX,
        Bloom {
            intensity: 0.28,
            composite_mode: BloomCompositeMode::EnergyConserving,
            ..default()
        },
        PanOrbitCamera {
            radius: Some(1.8 * n as f32),
            ..default()
        },
    ));

    // A separate full-window 2D camera owns the UI so the front-of-house overlays
    // stay full-screen even while the 3D camera above is shrunk to the hero rect
    // (Bevy UI is laid out against its camera's viewport). It composites on top of
    // the 3D (higher order, no clear) and is the default target for every UI node.
    commands.spawn((
        Camera2d,
        // Match the 3D camera's HDR-ness: a mix of HDR + non-HDR cameras breaks
        // rendering to magenta on Metal (bevy #6754). Both cameras Hdr + Msaa::Off.
        Hdr,
        Msaa::Off,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
    ));

    // ── lighting rig + horizon dome (DESIGN_BRIEF §2.2 / §3.8) ───────────
    lighting::spawn_rig(&mut commands, scale);
    environment::spawn_dome(&mut commands, &mut meshes, &mut materials, scale);

    // ── shared meshes + materials (DESIGN_BRIEF §3.1/§3.2/§3.3/§3.6) ─────
    let assets = RenderAssets {
        shell_mesh: meshes.add(Sphere::new(SHELL_RADIUS).mesh().ico(4).unwrap()),
        // Tiny empty-position marker sphere (half the previous size).
        dot_mesh: meshes.add(Sphere::new(0.06).mesh().ico(3).unwrap()),
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
        ring_mat: materials.add(materials::kaolin_ring()),
        empty_orb: orb_materials.add(orb::empty_material()),
        core_orb: [
            orb_materials.add(orb::material(Steward::Sienna)),
            orb_materials.add(orb::material(Steward::Lapis)),
            orb_materials.add(orb::material(Steward::Verdigris)),
            orb_materials.add(orb::material(Steward::Kaolin)),
        ],
    };

    // Hand every orb material to `hover.rs` so it can drive the selection uniform
    // (the sphere under the cursor swirls with light).
    let mut orb_handles = vec![assets.empty_orb.clone()];
    orb_handles.extend(assets.core_orb.iter().cloned());
    commands.insert_resource(orb::OrbHandles(orb_handles));

    // ── one persistent frame + core + ring entity per cell ──────────────
    let count = board.0.board.len();
    let mut frame = Vec::with_capacity(count);
    let mut core = Vec::with_capacity(count);
    let mut ring = Vec::with_capacity(count);
    let mut centers = Vec::with_capacity(count);
    for idx in 0..count {
        let pos = cell_world_pos(board.0.board.coord(idx), n);
        centers.push(pos);
        // Frame starts as the empty-position grey orb (matching `PrevStates`'
        // initial all-Empty), so sync_board only repaints cells that change.
        frame.push(
            commands
                .spawn((
                    Mesh3d(assets.dot_mesh.clone()),
                    MeshMaterial3d(assets.empty_orb.clone()),
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
                    MeshMaterial3d(assets.core_orb[0].clone()),
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

    // Tier-B life: pipes, agent motes, atari breath — tags each core entity and
    // seeds the per-cell animation parameters (DESIGN_BRIEF §3.4/§3.9/§4.9).
    effects::setup_effects(&mut commands, &mut meshes, &mut materials, n, count, &core);

    // Tier-C drama: one hidden per-cell mist volume + material (DESIGN_BRIEF §3.6).
    mist::setup_mist(&mut commands, &mut meshes, &mut mist_materials, &centers);

    commands.insert_resource(assets);
    commands.insert_resource(CellEntities { frame, core, ring });
    commands.insert_resource(PrevStates(vec![CellState::Empty; count]));
    commands.insert_resource(Transitions {
        became_live: vec![false; count],
    });
}

/// Rewrite the per-cell entities from the live board, when something changed, and
/// detect the §4.6 collapse / dispersal transitions that drive the mist and
/// cascade. Does *not* clear [`BoardDirty`] — [`clear_board_dirty`] does that last
/// so [`effects::sync_effects`] sees the same flag.
#[allow(clippy::too_many_arguments)]
fn sync_board(
    dirty: Res<BoardDirty>,
    time: Res<Time>,
    board: Res<BoardResource>,
    cells: Res<CellEntities>,
    assets: Res<RenderAssets>,
    mut prev: ResMut<PrevStates>,
    mut mist_state: ResMut<mist::MistState>,
    mut transitions: ResMut<Transitions>,
    mut commands: Commands,
) {
    if !dirty.0 {
        return;
    }
    let gs = &board.0;
    let now = time.elapsed_secs();
    // A fresh game (no placements yet) initialises mist at rest without playing
    // any transition; otherwise diff the previous states to find what changed.
    let fresh = gs.turn == 0;
    for f in transitions.became_live.iter_mut() {
        *f = false;
    }
    for idx in 0..gs.board.len() {
        let frame = cells.frame[idx];
        let core = cells.core[idx];
        let ring = cells.ring[idx];

        // Transition detection (drives mist + cascade) before repainting.
        let next = gs.board.get(idx);
        let was = prev.0[idx];
        if fresh {
            mist_state.reset_cell(idx, next);
        } else if next != was {
            mist_state.on_transition(idx, was, next, now);
            if matches!(next, CellState::Live(_)) {
                transitions.became_live[idx] = true;
            }
        }
        let changed = fresh || next != was;
        prev.0[idx] = next;
        // Only repaint cells whose state actually changed. Re-inserting the mesh
        // + material on all 125 cells every move re-prepares them in the render
        // world, which reads as a strong redraw flicker as new spheres come in.
        if !changed {
            continue;
        }
        // Kaolin's rim only shows for a live Kaolin core; default it off.
        let mut ring_visible = false;
        // The frame swaps material *type* between the empty cage (PlasmaMaterial)
        // and the live/dead shells (StandardMaterial), so each branch inserts one
        // and removes the other.
        match gs.board.get(idx) {
            // Empty → tiny clear-grey glass orb marking the position, core hidden.
            CellState::Empty => {
                commands
                    .entity(frame)
                    .insert((
                        Mesh3d(assets.dot_mesh.clone()),
                        MeshMaterial3d(assets.empty_orb.clone()),
                        Visibility::Visible,
                    ))
                    .remove::<MeshMaterial3d<StandardMaterial>>();
                commands.entity(core).insert(Visibility::Hidden);
            }
            // Live → marble: thick clear glass shell refracting an opaque neon
            // core inside it (§3.2/§3.3).
            CellState::Live(steward) => {
                commands
                    .entity(frame)
                    .insert((
                        Mesh3d(assets.shell_mesh.clone()),
                        MeshMaterial3d(assets.glass_mat.clone()),
                        Visibility::Visible,
                    ))
                    .remove::<MeshMaterial3d<OrbMaterial>>();
                commands.entity(core).insert((
                    MeshMaterial3d(assets.core_orb[steward.slot() as usize].clone()),
                    Visibility::Visible,
                ));
                ring_visible = steward == Steward::Kaolin;
            }
            // Temp-dead → darkened shell, core off (§3.6).
            CellState::TempDead(_) => {
                commands
                    .entity(frame)
                    .insert((
                        Mesh3d(assets.shell_mesh.clone()),
                        MeshMaterial3d(assets.tempdead_mat.clone()),
                        Visibility::Visible,
                    ))
                    .remove::<MeshMaterial3d<OrbMaterial>>();
                commands.entity(core).insert(Visibility::Hidden);
            }
            // Perma-dead → Verdigris-tinted shell, core off (§3.6).
            CellState::PermaDead => {
                commands
                    .entity(frame)
                    .insert((
                        Mesh3d(assets.shell_mesh.clone()),
                        MeshMaterial3d(assets.permadead_mat.clone()),
                        Visibility::Visible,
                    ))
                    .remove::<MeshMaterial3d<OrbMaterial>>();
                commands.entity(core).insert(Visibility::Hidden);
            }
        }
        commands.entity(ring).insert(if ring_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}

/// Clear [`BoardDirty`] after every dirty-driven sync system has run this frame.
fn clear_board_dirty(mut dirty: ResMut<BoardDirty>) {
    dirty.0 = false;
}
