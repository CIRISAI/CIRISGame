//! Fixed cubic embedding for the play lattice.
//!
//! Cells occupy every integer point of the `n³` box; connections are the six
//! axis-aligned face-neighbors (±x, ±y, ±z). There is no topology dial —
//! the 5×5×5 simple-cubic grid is the one and only layout.
//!
//! This module owns per-frame positioning of cell shells and tubes, and exposes
//! resources that the tuning panel and other systems read: [`PeerDistance`],
//! [`TubeWidth`], [`MarbleSize`], [`LatticeCell`].

use bevy::prelude::*;

use crate::effects::{CoreCell, PipeBirth, PipeEnds, PIPE_GROW_SECS, PIPE_TOTAL_LEN};
use crate::BoardResource;
use ciris_game_engine_core::Coord;

/// Global spacing multiplier between lattice nodes (the "peer distance" knob).
#[derive(Resource)]
pub(crate) struct PeerDistance(pub f32);

impl Default for PeerDistance {
    fn default() -> Self {
        PeerDistance(1.0)
    }
}

/// Global radius multiplier for the connecting tubes (the "tube width" knob).
#[derive(Resource)]
pub(crate) struct TubeWidth(pub f32);

impl Default for TubeWidth {
    fn default() -> Self {
        TubeWidth(1.0)
    }
}

/// Global scale multiplier for the marbles (the "marble size" knob). Cores fold
/// it into their breathing scale; shells / empty markers / rings take it directly.
#[derive(Resource)]
pub(crate) struct MarbleSize(pub f32);

impl Default for MarbleSize {
    fn default() -> Self {
        MarbleSize(1.0)
    }
}

/// Tags a per-cell entity (frame / core / ring) with its board index so
/// `position_cells` can read the corresponding coordinate.
#[derive(Component)]
pub(crate) struct LatticeCell(pub usize);

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<PeerDistance>()
        .init_resource::<TubeWidth>()
        .init_resource::<MarbleSize>()
        // Position AFTER sync_effects so a freshly-rebuilt pipe is re-fitted
        // to the correct cell positions the *same* frame.
        .add_systems(
            Update,
            (position_cells, position_pipes)
                .chain()
                .after(crate::effects::sync_effects),
        );
}

/// World-space position for a cell on the simple-cubic grid, centred at origin.
/// `peer` is the spacing multiplier from [`PeerDistance`].
pub(crate) fn cell_pos(c: Coord, n: u8) -> Vec3 {
    let half = (n as f32 - 1.0) / 2.0;
    Vec3::new(c.i as f32 - half, c.j as f32 - half, c.k as f32 - half)
}

/// Place every per-cell entity at its cubic-grid position each frame.
fn position_cells(
    board: Res<BoardResource>,
    peer: Res<PeerDistance>,
    marble: Res<MarbleSize>,
    mut q: Query<(&LatticeCell, Option<&CoreCell>, &mut Transform)>,
) {
    let n = board.0.board.n;
    for (cell, core, mut tf) in &mut q {
        tf.translation = cell_pos(board.0.board.coord(cell.0), n) * peer.0;
        // Cores fold marble size into their own breathing scale (breathe_cores);
        // shell / empty marker / ring take it directly here.
        if core.is_none() {
            tf.scale = Vec3::splat(marble.0);
        }
    }
}

/// Re-fit every glass tube between its two cells' cubic positions,
/// carrying the §4.6 grow-in animation.
fn position_pipes(
    time: Res<Time>,
    board: Res<BoardResource>,
    peer: Res<PeerDistance>,
    tube: Res<TubeWidth>,
    mut q: Query<(&PipeEnds, &PipeBirth, &mut Transform)>,
) {
    let n = board.0.board.n;
    let now = time.elapsed_secs();
    for (ends, birth, mut tf) in &mut q {
        let ca = board.0.board.coord(ends.a);
        let cb = board.0.board.coord(ends.b);
        let ea = cell_pos(ca, n) * peer.0;
        let eb = cell_pos(cb, n) * peer.0;
        let dir = eb - ea;
        let len = dir.length().max(1.0e-4);
        let grow = smooth(((now - birth.0) / PIPE_GROW_SECS).clamp(0.0, 1.0)).max(0.02);
        tf.translation = (ea + eb) * 0.5;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir / len);
        tf.scale = Vec3::new(tube.0, (len / PIPE_TOTAL_LEN) * grow, tube.0);
    }
}

/// Smooth Hermite ramp in [0, 1].
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
