//! Peer-spike hints: every placeable cell — live marbles AND the small empty
//! placement markers — sprouts short CLEAR-GLASS spikes pointing at its
//! face-neighbours (its *peers*, where a same-colour tube could form). Because
//! the spikes track the current embedding, they reveal which cells are actually
//! adjacent no matter how the topology dial warps the layout. When a cell is
//! selected, the spikes that point AT it light up — showing exactly what connects
//! to where you're about to play.

use bevy::prelude::*;

use crate::hover::SelectedCell;
use crate::orb::OrbMaterial;
use crate::render::{BLOOM_LAYER, SHELL_RADIUS};
use crate::topology::{embed, MarbleSize, PeerDistance, Topology};
use crate::BoardResource;
use ciris_game_engine_core::CellState;

/// Base spike geometry (scaled per-cell below).
const SPIKE_LEN: f32 = 0.20;
const SPIKE_R: f32 = 0.05;
/// Scale on a live marble vs. on a small empty placement marker.
const SCALE_LIVE: f32 = 0.5;
const SCALE_EMPTY: f32 = 0.28;
/// How far an empty-cell spike sits from the marker centre (hugs the marker).
const OUT_EMPTY: f32 = 0.05;
/// Face-neighbour count on the FCC lattice.
const MAX_NB: usize = 12;

#[derive(Component)]
struct Spike;

/// Spike entities per cell, indexed `[cell][slot]`.
#[derive(Resource)]
struct SpikeGrid(Vec<[Entity; MAX_NB]>);

/// Clear (resting) and hot (points-at-selected) spike materials.
#[derive(Resource)]
struct SpikeMats {
    clear: Handle<OrbMaterial>,
    hot: Handle<OrbMaterial>,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Update, orient_spikes.after(crate::effects::sync_effects));
}

/// A clear-glass spike material: opaque glass (like the marble edge — refracts
/// the nebulae, not the scene) with only a faint gas core, so it reads CLEAR, not
/// milky. `glow`/`core` push it to the bright "hot" variant when highlighted.
fn spike_material(rgb: LinearRgba, glow: f32, core: f32, sat: f32) -> OrbMaterial {
    OrbMaterial {
        color: LinearRgba::new(rgb.red, rgb.green, rgb.blue, 1.0),
        params: Vec4::new(0.15, 2.8, glow, 3.0),
        hover: Vec4::ZERO,
        params2: Vec4::new(sat, 0.0, 0.0, 0.0),
        glass: Vec4::new(1.45, core, 0.0, 0.6),
    }
}

/// Spawn the hidden spike pool (one set of 12 per cell). Called from
/// `render::setup` once the board size is known.
pub(crate) fn setup_spikes(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    orb_mats: &mut Assets<OrbMaterial>,
    count: usize,
) {
    let mesh = meshes.add(Cone {
        radius: SPIKE_R,
        height: SPIKE_LEN,
    });
    let mats = SpikeMats {
        clear: orb_mats.add(spike_material(
            LinearRgba::rgb(0.55, 0.68, 0.95),
            0.9,
            0.2,
            1.2,
        )),
        hot: orb_mats.add(spike_material(
            LinearRgba::rgb(1.0, 0.95, 0.85),
            5.0,
            0.55,
            1.4,
        )),
    };
    let mut grid = Vec::with_capacity(count);
    for _cell in 0..count {
        let mut arr = [Entity::PLACEHOLDER; MAX_NB];
        for slot in arr.iter_mut() {
            *slot = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mats.clear.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    bevy::camera::visibility::RenderLayers::from_layers(&[0, BLOOM_LAYER]),
                    Spike,
                ))
                .id();
        }
        grid.push(arr);
    }
    commands.insert_resource(SpikeGrid(grid));
    commands.insert_resource(mats);
}

/// Each frame, point every placeable cell's spikes at its in-bounds neighbours in
/// the current embedding; hide dead/empty-of-board cells; light spikes that point
/// at the selected cell.
#[allow(clippy::too_many_arguments)]
fn orient_spikes(
    board: Res<BoardResource>,
    topo: Res<Topology>,
    peer: Res<PeerDistance>,
    marble: Res<MarbleSize>,
    selected: Res<SelectedCell>,
    grid: Option<Res<SpikeGrid>>,
    mats: Option<Res<SpikeMats>>,
    mut q: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<OrbMaterial>,
        ),
        With<Spike>,
    >,
) {
    let (Some(grid), Some(mats)) = (grid, mats) else {
        return;
    };
    let n = board.0.board.n;
    for cell in 0..board.0.board.len() {
        let arr = &grid.0[cell];
        // Spikes hint connections from any PLACEABLE cell — live marbles and the
        // empty placement markers — but not dead cells (out of play).
        let live = matches!(board.0.board.get(cell), CellState::Live(_));
        let show = live || matches!(board.0.board.get(cell), CellState::Empty);
        if !show {
            for &e in arr {
                if let Ok((_, mut vis, _)) = q.get_mut(e) {
                    if *vis != Visibility::Hidden {
                        *vis = Visibility::Hidden;
                    }
                }
            }
            continue;
        }
        let cpos = embed(board.0.board.coord(cell), n, &topo) * peer.0;
        let nbrs = board.0.board.neighbors(cell);
        let scale = if live { SCALE_LIVE } else { SCALE_EMPTY };
        // Live spikes start just outside the marble surface; empty ones hug the
        // small marker.
        let out = if live {
            SHELL_RADIUS * marble.0 + 0.03
        } else {
            OUT_EMPTY
        };
        for (slot, &e) in arr.iter().enumerate() {
            let Ok((mut tf, mut vis, mut matc)) = q.get_mut(e) else {
                continue;
            };
            if slot >= nbrs.len() {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
                continue;
            }
            let target = nbrs[slot];
            let npos = embed(board.0.board.coord(target), n, &topo) * peer.0;
            let dir = (npos - cpos).normalize_or_zero();
            if dir == Vec3::ZERO {
                *vis = Visibility::Hidden;
                continue;
            }
            tf.translation = cpos + dir * (out + SPIKE_LEN * scale * 0.5);
            tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
            tf.scale = Vec3::splat(scale);
            *vis = Visibility::Visible;
            // Light the spikes that point AT the selected cell.
            let want = if selected.0 == Some(target) {
                &mats.hot
            } else {
                &mats.clear
            };
            if matc.0.id() != want.id() {
                matc.0 = want.clone();
            }
        }
    }
}
