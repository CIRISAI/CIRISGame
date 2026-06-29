//! Tier-B life on top of the static lattice: fat swirling-gas pipes between
//! same-steward neighbours (DESIGN_BRIEF §3.4) and the atari breath (§4.9).
//! Feature-gated behind `render`.
//!
//! (Orbiting agent motes were removed for now to keep the base clean — the
//! sphere interior is swirling gas, not orbiting particles; motes return later.)
//!
//! Driven *from* `GameState`: nothing here decides game state. [`sync_effects`]
//! runs after `sync_board` on every [`BoardDirty`] and rebuilds the per-cell
//! animation parameters ([`CellAnim`]), the glass pipes, and the atari rings.
//! [`breathe_cores`] / [`grow_pipes`] then read those parameters plus `Time`.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::orb::OrbMaterial;
use crate::render::{BoardDirty, Transitions, SHELL_RADIUS};
use crate::{materials, BoardResource};
use ciris_game_engine_core::{CellState, Steward, ATARI_SIZE};

/// Outer glass-tube radius (DESIGN_BRIEF §3.4) — a substantial glass neck joining
/// two connected spheres, the *same* glass as the sphere shells.
const PIPE_RADIUS: f32 = 0.16;
/// Tube length — face-neighbour centres sit √2 ≈ 1.414 apart (§3.1). We span most
/// of that so the tube drives *into* both spheres (no gap), reading as one joined
/// ball-and-stick object rather than a strut floating between them. `pub(crate)`
/// so `topology` can rescale the tube to the embedded endpoint distance.
pub(crate) const PIPE_LEN: f32 = 0.95;
/// The capsule's TRUE total length (cylinder + the two hemispherical end-caps).
/// Scaling must divide by this, not `PIPE_LEN`, or the caps make the tube ~34 %
/// too long and it spikes out past the spheres it joins. `pub(crate)` for
/// `topology::position_pipes`.
pub(crate) const PIPE_TOTAL_LEN: f32 = PIPE_LEN + 2.0 * PIPE_RADIUS;

/// Atari breath frequency in Hz (DESIGN_BRIEF §4.9).
const BREATH_HZ: f32 = 0.6;
/// Atari core scale breath — the visible inhale/exhale of the held-breath cell.
/// Small enough that the scaled core stays inside its glass shell.
const BREATH_SCALE_AMP: f32 = 0.06;

/// Core fade-in duration when a cell becomes live, e.g. the §4.6 dispersal
/// rebuild "cores reappear" beat (also a gentle pop-in for ordinary placements).
const CORE_BIRTH_SECS: f32 = 0.5;
/// New-pipe extrude duration along the channel (DESIGN_BRIEF §4.6). `pub(crate)`
/// so `topology::position_pipes` can carry the grow-in while it re-fits the tube.
pub(crate) const PIPE_GROW_SECS: f32 = 0.4;
/// A birth timestamp far enough in the past to read as "fully grown".
const BORN_LONG_AGO: f32 = -1000.0;

/// Per-cell animation parameters, rebuilt from `GameState` on every
/// [`BoardDirty`] and read each frame by [`breathe_cores`].
#[derive(Clone, Copy)]
struct CellAnimEntry {
    /// World-space cell centre (constant, but cached here for the hot path).
    center: Vec3,
    /// True when the cell is a live steward core.
    live: bool,
    /// True when this cell belongs to a mesh in atari (`|M| = ATARI_SIZE`).
    atari: bool,
    /// `Time::elapsed_secs` when this core last became live; drives the §4.6
    /// fade-in. [`BORN_LONG_AGO`] means "already settled".
    birth: f32,
}

impl Default for CellAnimEntry {
    fn default() -> Self {
        CellAnimEntry {
            center: Vec3::ZERO,
            live: false,
            atari: false,
            birth: BORN_LONG_AGO,
        }
    }
}

/// All per-cell animation parameters, indexed by linear board index.
#[derive(Resource)]
pub(crate) struct CellAnim(Vec<CellAnimEntry>);

/// Live-tunable global multiplier on the steward core size (driven by the tuning
/// panel; 1.0 = the modelled radius).
#[derive(Resource)]
pub(crate) struct CoreScale(pub f32);

impl Default for CoreScale {
    fn default() -> Self {
        CoreScale(1.0)
    }
}

/// Shared, immutable effect handles built once at startup. Pipe gas materials are
/// *not* shared — each pipe gets its own [`pipe::PipeMaterial`] in [`sync_effects`].
#[derive(Resource)]
pub(crate) struct EffectAssets {
    /// The tube mesh — drawn with the SAME orb glass-marble material as the
    /// spheres (clear glass + gas core + world-box refraction), so a bond reads
    /// as a glass tube of the steward's gas, opaque to everything behind it.
    pipe_glass_mesh: Handle<Mesh>,
    /// Steward orb (glass-marble) material per slot — the tube's glass + gas.
    tube_orb: [Handle<OrbMaterial>; 4],
}

/// Dynamic effect entities owned by [`sync_effects`].
#[derive(Resource)]
pub(crate) struct EffectState {
    /// Live glass-pipe entities, despawned + rebuilt each [`BoardDirty`].
    pipes: Vec<Entity>,
    /// Verdigris foreshadowing-ring entity per cell (shown only when atari).
    atari_rings: Vec<Entity>,
}

/// Marks a steward-core entity so the atari breath can scale it. Carries the
/// cell's linear index for the [`CellAnim`] lookup.
#[derive(Component)]
pub(crate) struct CoreCell(pub usize);

/// A glass pipe's spawn time, so the grow-in can extrude it (DESIGN_BRIEF §4.6).
/// Pipes that existed before the move are born "long ago" and spawn full length.
#[derive(Component)]
pub(crate) struct PipeBirth(pub f32);

/// A straight tube between two cells, so [`crate::topology`] can re-fit it every
/// frame as the embedding morphs. Runs end-to-end between the two sphere centres.
#[derive(Component)]
pub(crate) struct PipeEnds {
    pub a: usize,
    pub b: usize,
}

/// Build the shared effect assets and the atari rings, and seed [`CellAnim`].
/// Called from `render::setup` once the per-cell entity table exists; `cores` is
/// that table's core-entity list (one per cell, in index order) so we can tag
/// each with [`CoreCell`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn setup_effects(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tube_orb: [Handle<OrbMaterial>; 4],
    centers: &[Vec3],
    cores: &[Entity],
) {
    let pipe_glass_mesh = meshes.add(Capsule3d::new(PIPE_RADIUS, PIPE_LEN));

    let ring_mesh = meshes.add(Torus {
        major_radius: SHELL_RADIUS,
        minor_radius: 0.018,
    });
    let ring_mat = materials.add(materials::atari_ring());

    let count = centers.len();
    let mut anim = vec![CellAnimEntry::default(); count];
    let mut atari_rings = Vec::with_capacity(count);
    for idx in 0..count {
        let center = centers[idx];
        anim[idx].center = center;

        // Tag this cell's core for the breath system.
        commands.entity(cores[idx]).insert(CoreCell(idx));

        // One foreshadowing ring per cell, hidden until the cell enters atari.
        atari_rings.push(
            commands
                .spawn((
                    Mesh3d(ring_mesh.clone()),
                    MeshMaterial3d(ring_mat.clone()),
                    Transform::from_translation(center),
                    Visibility::Hidden,
                ))
                .id(),
        );
    }

    commands.insert_resource(CellAnim(anim));
    commands.insert_resource(EffectAssets {
        pipe_glass_mesh,
        tube_orb,
    });
    commands.insert_resource(EffectState {
        pipes: Vec::new(),
        atari_rings,
    });
}

/// Rebuild the per-cell animation parameters, the glass pipes, and the atari
/// rings from the live board. Runs after `sync_board` on every [`BoardDirty`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_effects(
    dirty: Res<BoardDirty>,
    time: Res<Time>,
    board: Res<BoardResource>,
    transitions: Res<Transitions>,
    assets: Res<EffectAssets>,
    mut anim: ResMut<CellAnim>,
    mut state: ResMut<EffectState>,
    mut commands: Commands,
) {
    // `sync_board` clears `BoardDirty`; rerun this whenever the parameters are
    // stale. We mirror its dirty read instead of clearing it ourselves, so order
    // the two with `sync_board` *before* `sync_effects` (see `run_app`).
    if !dirty.0 {
        return;
    }
    let gs = &board.0;
    let b = &gs.board;

    // Precompute, per cell, its mesh size (for atari detection) via a stable mesh
    // id so adjacent meshes are counted once each.
    let count = b.len();
    let mut mesh_id = vec![usize::MAX; count];
    let mut mesh_size: Vec<usize> = Vec::new();
    for steward in Steward::ALL {
        for mesh in b.meshes_of(steward) {
            let id = mesh_size.len();
            mesh_size.push(mesh.len());
            for c in mesh {
                mesh_id[c] = id;
            }
        }
    }

    for idx in 0..count {
        let mut e = CellAnimEntry {
            center: anim.0[idx].center,
            ..Default::default()
        };
        if let CellState::Live(_) = b.get(idx) {
            e.live = true;
            e.atari = mesh_size[mesh_id[idx]] == ATARI_SIZE;
        }
        // Stamp a birth time for cores that just came live (§4.6 fade-in); carry
        // the prior time forward for cores that were already settled.
        e.birth = if transitions.became_live[idx] {
            time.elapsed_secs()
        } else {
            anim.0[idx].birth
        };
        anim.0[idx] = e;

        // Foreshadowing ring follows atari state.
        let ring = state.atari_rings[idx];
        commands.entity(ring).insert(if anim.0[idx].atari {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }

    // Rebuild glass pipes: one per face-adjacent same-steward live pair (§3.4).
    for e in state.pipes.drain(..) {
        commands.entity(e).despawn();
    }
    for idx in 0..count {
        let CellState::Live(steward) = b.get(idx) else {
            continue;
        };
        let ca = anim.0[idx].center;
        for nb in b.neighbors(idx) {
            if nb <= idx {
                continue;
            }
            if b.get(nb) != CellState::Live(steward) {
                continue;
            }
            let cb = anim.0[nb].center;
            let dir = (cb - ca).normalize();
            let center = (ca + cb) * 0.5;
            // A pipe touching a freshly-live cell extrudes from nothing; pipes
            // between long-settled cells spawn at full length (§4.6).
            let born = transitions.became_live[idx] || transitions.became_live[nb];
            let birth = if born {
                time.elapsed_secs()
            } else {
                BORN_LONG_AGO
            };
            let start_len = if born { 0.02 } else { 1.0 };
            let transform = Transform {
                translation: center,
                rotation: Quat::from_rotation_arc(Vec3::Y, dir),
                scale: Vec3::new(1.0, start_len, 1.0),
            };
            // Unified with the spheres: ONE straight tube of the same orb
            // glass-marble material (clear glass + gas core + starfield
            // refraction, opaque). The no-crossing rule (§4.11) guarantees
            // different-colour bonds never cross, so no bow/bend is needed —
            // re-fit each frame by `topology::position_pipes`.
            let slot = steward.slot() as usize;
            let tube = commands
                .spawn((
                    Mesh3d(assets.pipe_glass_mesh.clone()),
                    MeshMaterial3d(assets.tube_orb[slot].clone()),
                    transform,
                    PipeBirth(birth),
                    PipeEnds { a: idx, b: nb },
                    bevy::camera::visibility::RenderLayers::from_layers(&[
                        0,
                        crate::render::BLOOM_LAYER,
                    ]),
                ))
                .id();
            state.pipes.push(tube);
        }
    }
}

/// Pulse the atari cells' cores: a 0.6 Hz inhale/exhale scale breath
/// (DESIGN_BRIEF §4.9). Live non-atari cores rest at unit scale; hidden cores are
/// left alone. Also runs the §4.6 birth fade-in.
pub(crate) fn breathe_cores(
    time: Res<Time>,
    anim: Res<CellAnim>,
    core_scale: Res<CoreScale>,
    marble: Res<crate::topology::MarbleSize>,
    mut cores: Query<(&CoreCell, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let breath = 1.0 + BREATH_SCALE_AMP * (TAU * BREATH_HZ * t).sin();
    for (core, mut tf) in &mut cores {
        let e = anim.0[core.0];
        if !e.live {
            continue;
        }
        let s = if e.atari { breath } else { 1.0 };
        // Fade-in: scale the core up from a point over CORE_BIRTH_SECS when it
        // first comes live (§4.6 dispersal "cores reappear").
        let birth = smoothstep01((t - e.birth) / CORE_BIRTH_SECS);
        tf.scale = Vec3::splat(s * birth * core_scale.0 * marble.0);
    }
}

/// Smooth Hermite ramp clamped to `[0, 1]`.
fn smoothstep01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
