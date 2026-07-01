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

/// Layer-slicer state: click to peel successive Y-layers off the top so
/// interior cells become visible.  0 = closed cube, 1-4 = layers peeled.
#[derive(Resource, Default)]
pub(crate) struct LayerSlicer {
    /// Target number of layers currently peeled (0–4).
    pub state: u8,
    /// Smoothly-animated fractional value approaching `state`.
    pub anim: f32,
}

/// Y gap between the peeled group and the base group (world units, then
/// multiplied by PeerDistance so the ratio stays constant as the board scales).
const GAP: f32 = 3.5;

/// Marker on the hamburger button so `slicer_click` can find it.
#[derive(Component)]
struct SlicerButton;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<PeerDistance>()
        .init_resource::<TubeWidth>()
        .init_resource::<MarbleSize>()
        .init_resource::<LayerSlicer>()
        .add_systems(Startup, spawn_slicer)
        // animate_slicer + slicer_click run first, then positioning uses the
        // updated anim value — all after sync_effects.
        .add_systems(
            Update,
            (animate_slicer, slicer_click, position_cells, position_pipes)
                .chain()
                .after(crate::effects::sync_effects),
        );
}

/// Spawn a small hamburger button (3 horizontal bars) in the top-left.
/// Clicking cycles the slicer through states 0 → 1 → 2 → 3 → 4 → 0.
fn spawn_slicer(mut commands: Commands) {
    let btn = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.10, 0.13, 0.85)),
            GlobalZIndex(60),
            SlicerButton,
        ))
        .id();
    for _ in 0..3 {
        commands.spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(3.0),
                border_radius: BorderRadius::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.82, 0.84, 0.92)),
            ChildOf(btn),
        ));
    }
}

/// Click the hamburger to cycle through slicer states 0–4 and back to 0.
fn slicer_click(
    q: Query<&Interaction, (Changed<Interaction>, With<SlicerButton>)>,
    mut slicer: ResMut<LayerSlicer>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            slicer.state = (slicer.state + 1) % 5;
        }
    }
}

/// Smoothly interpolate `anim` toward `state` (exponential decay, ~9 rad/s).
fn animate_slicer(mut slicer: ResMut<LayerSlicer>, time: Res<Time>) {
    let target = slicer.state as f32;
    let k = (1.0 - (-time.delta_secs() * 9.0).exp()).clamp(0.0, 1.0);
    slicer.anim += (target - slicer.anim) * k;
}

/// World-space position for a cell on the simple-cubic grid, centred at origin.
pub(crate) fn cell_pos(c: Coord, n: u8) -> Vec3 {
    let half = (n as f32 - 1.0) / 2.0;
    Vec3::new(c.i as f32 - half, c.j as f32 - half, c.k as f32 - half)
}

/// Extra Y offset for layer-slicing: layer `j` smoothly lifts as `anim`
/// passes through its threshold.  Layer j=n-1 lifts first (at anim=1), then
/// j=n-2 (anim=2), …, j=1 (anim=4).  Layer j=0 never lifts.
fn lift_y(j: u8, n: u8, anim: f32) -> f32 {
    let threshold = (n as f32 - 1.0) - j as f32;
    let t = (anim - threshold).clamp(0.0, 1.0);
    smooth(t) * GAP
}

/// Place every per-cell entity at its cubic-grid + slicer-lifted position.
fn position_cells(
    board: Res<BoardResource>,
    peer: Res<PeerDistance>,
    marble: Res<MarbleSize>,
    slicer: Res<LayerSlicer>,
    mut q: Query<(&LatticeCell, Option<&CoreCell>, &mut Transform)>,
) {
    let n = board.0.board.n;
    for (cell, core, mut tf) in &mut q {
        let c = board.0.board.coord(cell.0);
        let mut pos = cell_pos(c, n) * peer.0;
        pos.y += lift_y(c.j, n, slicer.anim) * peer.0;
        tf.translation = pos;
        if core.is_none() {
            tf.scale = Vec3::splat(marble.0);
        }
    }
}

/// Re-fit every glass tube between its two (possibly lifted) cells.
fn position_pipes(
    time: Res<Time>,
    board: Res<BoardResource>,
    peer: Res<PeerDistance>,
    tube: Res<TubeWidth>,
    slicer: Res<LayerSlicer>,
    mut q: Query<(&PipeEnds, &PipeBirth, &mut Transform)>,
) {
    let n = board.0.board.n;
    let now = time.elapsed_secs();
    for (ends, birth, mut tf) in &mut q {
        let ca = board.0.board.coord(ends.a);
        let cb = board.0.board.coord(ends.b);
        let mut ea = cell_pos(ca, n) * peer.0;
        let mut eb = cell_pos(cb, n) * peer.0;
        ea.y += lift_y(ca.j, n, slicer.anim) * peer.0;
        eb.y += lift_y(cb.j, n, slicer.anim) * peer.0;
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
