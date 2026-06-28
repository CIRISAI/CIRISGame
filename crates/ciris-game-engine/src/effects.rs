//! Tier-B life on top of the static lattice: glass pipes between same-steward
//! neighbours (DESIGN_BRIEF §3.4), orbiting agent motes (§3.9 / §4.3), and the
//! atari breath (§4.9). Everything is feature-gated behind `render`.
//!
//! Like the rest of the view layer this is driven *from* `GameState`: nothing
//! here ever decides game state. [`sync_effects`] runs after [`crate::render`]'s
//! `sync_board` on every [`BoardDirty`] and rebuilds the per-cell animation
//! parameters ([`CellAnim`]), the glass pipes, and the atari rings. The two
//! per-frame systems ([`animate_motes`], [`breathe_cores`]) then read those
//! parameters plus `Time` to move the motes along their orbits and pulse the
//! atari breath — they touch no game state at all.

use std::f32::consts::TAU;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::render::{cell_world_pos, BoardDirty, Transitions, BLOOM_LAYER, SHELL_RADIUS};
use crate::{materials, pipe, BoardResource};
use ciris_game_engine_core::{temperature, CellState, Steward, ATARI_SIZE};

/// Glass pipe radius (DESIGN_BRIEF §3.4).
const PIPE_RADIUS: f32 = 0.055;
/// Glass pipe length — the channel between two shells. Face-neighbour centres sit
/// √2 apart (§3.1); subtract the two shell radii: √2 − 2·0.42 ≈ 0.574 (§3.4).
const PIPE_LEN: f32 = 0.574;

/// Mote sphere radius. Small enough to read as a glint through the §2.3 bloom.
const MOTE_RADIUS: f32 = 0.022;
/// Hard cap on motes per node (DESIGN_BRIEF §3.9).
const MOTE_MAX: u8 = 5;
/// Rest orbit radius as a fraction of the shell radius (DESIGN_BRIEF §4.3).
const ORBIT_R_REST: f32 = 0.62;
/// Atari orbit radius fraction — particles pull inward (DESIGN_BRIEF §3.9).
const ORBIT_R_ATARI: f32 = 0.50;
/// Wobble displacement (along the orbit axis) at full temperature (§4.3).
const WOBBLE_AMP: f32 = 0.06;
/// Wobble temporal frequency (rad/s) for the pseudo-noise jitter.
const WOBBLE_FREQ: f32 = 2.7;

/// Atari breath frequency in Hz (DESIGN_BRIEF §4.9).
const BREATH_HZ: f32 = 0.6;
/// Atari emissive breath amplitude, ±0.4 around the base (DESIGN_BRIEF §4.9).
const BREATH_EMISSIVE_AMP: f32 = 0.4;
/// Atari core scale breath — the visible inhale/exhale of the held-breath cell.
/// Small enough that the scaled core stays inside its glass shell.
const BREATH_SCALE_AMP: f32 = 0.06;
/// Atari orbit angular speed (rad/s), slower than calm (DESIGN_BRIEF §3.9).
const ATARI_OMEGA: f32 = 0.35;

/// Core fade-in duration when a cell becomes live, e.g. the §4.6 dispersal
/// rebuild "cores reappear" beat (also a gentle pop-in for ordinary placements).
const CORE_BIRTH_SECS: f32 = 0.5;
/// New-pipe extrude duration along the channel (DESIGN_BRIEF §4.6).
const PIPE_GROW_SECS: f32 = 0.4;
/// A birth timestamp far enough in the past to read as "fully grown".
const BORN_LONG_AGO: f32 = -1000.0;

/// Per-cell animation parameters, rebuilt from `GameState` on every
/// [`BoardDirty`] and read each frame by the animation systems.
#[derive(Clone, Copy)]
struct CellAnimEntry {
    /// World-space cell centre (constant, but cached here for the hot path).
    center: Vec3,
    /// True when the cell is a live steward core.
    live: bool,
    /// Steward slot 0..=3 (meaningless unless `live`).
    slot: usize,
    /// Active mote count for this node (0 unless `live`).
    motes: u8,
    /// Orbit angular speed, rad/s.
    omega: f32,
    /// Normalized mesh temperature in `[0, 1]` (§4.1), scales the wobble.
    t_vis: f32,
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
            slot: 0,
            motes: 0,
            omega: 0.6,
            t_vis: 0.0,
            atari: false,
            birth: BORN_LONG_AGO,
        }
    }
}

/// All per-cell animation parameters, indexed by linear board index.
#[derive(Resource)]
pub(crate) struct CellAnim(Vec<CellAnimEntry>);

/// Shared, immutable effect handles built once at startup. The liquid pipe
/// material is *not* shared — each pipe gets its own [`pipe::PipeMaterial`]
/// instance in [`sync_effects`] so the gravity fill measures against that pipe's
/// own world centre and extent.
#[derive(Resource)]
pub(crate) struct EffectAssets {
    pipe_mesh: Handle<Mesh>,
}

/// Dynamic effect entities owned by [`sync_effects`].
#[derive(Resource)]
pub(crate) struct EffectState {
    /// Live glass-pipe entities, despawned + rebuilt each [`BoardDirty`].
    pipes: Vec<Entity>,
    /// Verdigris foreshadowing-ring entity per cell (shown only when atari).
    atari_rings: Vec<Entity>,
}

/// A single orbiting mote. `axis` is its orbit-plane normal and `phase` its fixed
/// angular offset, both assigned deterministically at startup so the few motes
/// per node spread out instead of stacking.
#[derive(Component)]
pub(crate) struct Mote {
    cell: usize,
    k: u8,
    axis: Vec3,
    phase: f32,
}

/// Marks a steward-core entity so the atari breath can scale it. Carries the
/// cell's linear index for the [`CellAnim`] lookup.
#[derive(Component)]
pub(crate) struct CoreCell(pub usize);

/// A glass pipe, carrying the time it was spawned so [`grow_pipes`] can extrude
/// it along its length (DESIGN_BRIEF §4.6). Pipes that existed before the move are
/// born "long ago" and spawn at full length.
#[derive(Component)]
pub(crate) struct PipeBirth(f32);

/// Build the shared effect assets, the mote pool, and the atari rings, and seed
/// [`CellAnim`]. Called from `render::setup` once the per-cell entity table
/// exists; `cores` is that table's core-entity list (one per cell, in index
/// order) so we can tag each with [`CoreCell`].
pub(crate) fn setup_effects(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    n: u8,
    count: usize,
    cores: &[Entity],
) {
    let pipe_mesh = meshes.add(Capsule3d::new(PIPE_RADIUS, PIPE_LEN));

    let mote_mesh = meshes.add(Sphere::new(MOTE_RADIUS).mesh().ico(1).unwrap());
    let ring_mesh = meshes.add(Torus {
        major_radius: SHELL_RADIUS,
        minor_radius: 0.018,
    });
    let ring_mat = materials.add(materials::atari_ring());

    let mut anim = vec![CellAnimEntry::default(); count];
    let mut atari_rings = Vec::with_capacity(count);
    for idx in 0..count {
        let center = cell_world_pos(coord_of(idx, n), n);
        anim[idx].center = center;

        // Tag this cell's core for the breath system.
        commands.entity(cores[idx]).insert(CoreCell(idx));

        // A small pool of motes per cell, hidden until the cell goes live. Each
        // mote owns its own material so the atari breath can pulse it alone.
        for k in 0..MOTE_MAX {
            // Golden-angle tilt + per-cell jitter so orbits don't all share a plane.
            let a = k as f32 * 2.399_963 + idx as f32 * 0.37;
            let axis = Vec3::new(a.cos(), 0.6, a.sin()).normalize();
            let phase = k as f32 * (TAU / MOTE_MAX as f32) + idx as f32 * 0.41;
            commands.spawn((
                Mesh3d(mote_mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    unlit: true,
                    ..default()
                })),
                Transform::from_translation(center),
                Visibility::Hidden,
                RenderLayers::from_layers(&[0, BLOOM_LAYER]),
                Mote {
                    cell: idx,
                    k,
                    axis,
                    phase,
                },
            ));
        }

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
    commands.insert_resource(EffectAssets { pipe_mesh });
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
    mut pipe_materials: ResMut<Assets<pipe::PipeMaterial>>,
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

    // Precompute, per cell, its mesh size and a stable mesh id (so adjacent enemy
    // meshes are counted once each), via the core's component labelling.
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
        if let CellState::Live(steward) = b.get(idx) {
            let m = mesh_size[mesh_id[idx]];
            // Distinct adjacent enemy meshes → temperature inputs (§4.1).
            let mut enemy_ids: Vec<usize> = Vec::new();
            let mut links = 0u8;
            for nb in b.neighbors(idx) {
                match b.get(nb) {
                    CellState::Live(s) if s == steward => links = links.saturating_add(1),
                    CellState::Live(_) => {
                        let id = mesh_id[nb];
                        if !enemy_ids.contains(&id) {
                            enemy_ids.push(id);
                        }
                    }
                    _ => {}
                }
            }
            let enemy_sizes: Vec<usize> = enemy_ids.iter().map(|&id| mesh_size[id]).collect();
            let t_vis = temperature::t_vis(temperature::temperature(m, &enemy_sizes)) as f32;
            let atari = m == ATARI_SIZE;

            e.live = true;
            e.slot = steward.slot() as usize;
            e.t_vis = t_vis;
            e.atari = atari;
            // Count: 3 + min(9, links), hard cap 5; atari locks to 5 (§3.9).
            e.motes = if atari {
                MOTE_MAX
            } else {
                (3 + links.min(9)).min(MOTE_MAX)
            };
            e.omega = if atari {
                ATARI_OMEGA
            } else {
                0.6 + 0.25 * t_vis
            };
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
            let material = pipe_materials.add(pipe::material(steward));
            let pipe = commands
                .spawn((
                    Mesh3d(assets.pipe_mesh.clone()),
                    MeshMaterial3d(material),
                    transform,
                    PipeBirth(birth),
                ))
                .id();
            state.pipes.push(pipe);
        }
    }
}

/// Move every visible mote along its orbit and, in atari, pulse its emissive
/// breath. Runs every frame (DESIGN_BRIEF §4.3 / §4.9).
pub(crate) fn animate_motes(
    time: Res<Time>,
    anim: Res<CellAnim>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut motes: Query<(
        &Mote,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let t = time.elapsed_secs();
    // Global breath phase, phase-locked across all atari meshes (§4.9).
    let breath = 1.0 + BREATH_EMISSIVE_AMP * (TAU * BREATH_HZ * t).sin();

    for (mote, mut tf, mut vis, mat) in &mut motes {
        let e = anim.0[mote.cell];
        let active = e.live && mote.k < e.motes;
        if !active {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        if *vis != Visibility::Visible {
            *vis = Visibility::Visible;
        }

        let radius = if e.atari { ORBIT_R_ATARI } else { ORBIT_R_REST } * SHELL_RADIUS;
        let angle = t * e.omega + mote.phase;
        let (u, v) = mote.axis.any_orthonormal_pair();
        let ring = radius * (angle.cos() * u + angle.sin() * v);
        let wobble = WOBBLE_AMP * e.t_vis * (t * WOBBLE_FREQ + mote.phase * 2.0).sin();
        tf.translation = e.center + ring + mote.axis * wobble;

        if let Some(mut material) = materials.get_mut(mat.id()) {
            let factor = if e.atari { breath } else { 1.0 };
            material.emissive = materials::mote_emissive(e.slot, factor);
        }
    }
}

/// Pulse the atari cells' cores: a 0.6 Hz inhale/exhale scale breath, phase-locked
/// with the mote emissive breath (DESIGN_BRIEF §4.9). Live non-atari cores rest at
/// unit scale; hidden cores are left alone.
pub(crate) fn breathe_cores(
    time: Res<Time>,
    anim: Res<CellAnim>,
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
        tf.scale = Vec3::splat(s * birth);
    }
}

/// Extrude newly-spawned pipes along their length over [`PIPE_GROW_SECS`]
/// (DESIGN_BRIEF §4.6). Pipes born "long ago" are already at full length.
pub(crate) fn grow_pipes(time: Res<Time>, mut pipes: Query<(&PipeBirth, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (birth, mut tf) in &mut pipes {
        let g = smoothstep01((t - birth.0) / PIPE_GROW_SECS).max(0.02);
        if (tf.scale.y - g).abs() > 1.0e-4 {
            tf.scale.y = g;
        }
    }
}

/// Smooth Hermite ramp clamped to `[0, 1]`.
fn smoothstep01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Linear board index → `Coord`, without borrowing the board (the centre cache is
/// seeded before the board resource is read in the hot path).
fn coord_of(idx: usize, n: u8) -> ciris_game_engine_core::Coord {
    let n = n as usize;
    ciris_game_engine_core::Coord::new(
        (idx % n) as u8,
        ((idx / n) % n) as u8,
        (idx / (n * n)) as u8,
    )
}
