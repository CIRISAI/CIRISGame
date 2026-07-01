//! Fixed cubic embedding for the play lattice, with a live pincushion blend.
//!
//! Cells occupy every integer point of the `n³` box; connections are the six
//! axis-aligned face-neighbors (±x, ±y, ±z).
//!
//! This module owns per-frame positioning of cell shells and tubes, and exposes
//! resources that the tuning panel and other systems read: [`PeerDistance`],
//! [`TubeWidth`], [`MarbleSize`], [`LatticeCell`], [`TopoBlend`].

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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

/// Blend between cube layout (0.0) and pincushion layout (1.0).
/// At 1.0 the middle Y-layer expands 1.6× in XZ; outer layers compress to 0.9×.
#[derive(Resource)]
pub(crate) struct TopoBlend(pub f32);

impl Default for TopoBlend {
    fn default() -> Self {
        TopoBlend(0.0) // default: cube
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

/// Track width in logical pixels for the topo slider.
const SLIDER_TRACK_W: f32 = 88.0;
/// Extra panel width beyond the track: 2×7 px h-padding + 2×5 px col-gap + ~16 px for "C"/"P" labels.
const SLIDER_PANEL_GUTTER: f32 = 40.0;
/// Half the thumb diameter — used to clamp the thumb so it stays within the track bar.
const THUMB_HALF: f32 = 6.0;

/// Marker on the hamburger button so `slicer_click` can find it.
#[derive(Component)]
struct SlicerButton;

/// Marker on the topo-blend slider *panel* (the wide interactable Button).
#[derive(Component)]
struct TopoTrack;

/// Marker on the 88 px visual track bar — `normalize_point` is called against
/// this node so the blend maps cleanly to [0, 1] over the visible bar.
#[derive(Component)]
struct TopoTrackVisual;

/// Marker on the topo-blend slider thumb.
#[derive(Component)]
struct TopoThumb;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<PeerDistance>()
        .init_resource::<TubeWidth>()
        .init_resource::<MarbleSize>()
        .init_resource::<LayerSlicer>()
        .init_resource::<TopoBlend>()
        .add_systems(Startup, spawn_controls)
        // animate_slicer + slicer_click + slider run first, then positioning uses
        // updated values — all after sync_effects.
        .add_systems(
            Update,
            (
                animate_slicer,
                slicer_click,
                topo_slider_update,
                update_topo_thumb,
                position_cells,
                position_pipes,
            )
                .chain()
                .after(crate::effects::sync_effects),
        );
}

/// Spawn the hamburger slicer button and the topo-blend slider side by side.
fn spawn_controls(mut commands: Commands) {
    // ── layer-slicer button: 1 "peel" line floating above + 4 layer lines ──
    // The lone top line reads as the layer about to be peeled off; clicking
    // lifts it and reveals the next, so the icon telegraphs the action.
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
                padding: UiRect::vertical(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.10, 0.13, 0.85)),
            GlobalZIndex(60),
            SlicerButton,
        ))
        .id();
    // Floating top line (the "peel" indicator) — brighter, with a gap below.
    commands.spawn((
        Node {
            width: Val::Px(20.0),
            height: Val::Px(2.5),
            border_radius: BorderRadius::all(Val::Px(1.5)),
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 0.90, 0.55)), // warm highlight = "lifted layer"
        ChildOf(btn),
    ));
    // Four layer lines — same width, tight row_gap, representing the 4 peel-able layers.
    for _ in 0..4 {
        commands.spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(2.5),
                border_radius: BorderRadius::all(Val::Px(1.5)),
                margin: UiRect::vertical(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.82, 0.84, 0.92)),
            ChildOf(btn),
        ));
    }

    // ── topo-blend slider (Cube ←→ Pinch) right of the hamburger ───────────
    // The whole panel is the Button / interactable so the hit target is the full
    // 44 px height. Non-Button children pass focus implicitly in Bevy 0.19's
    // picking backend (FocusPolicy is ignored by UiPickingPlugin).
    let panel = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(68.0), // 16 + 44 + 8 gap
                height: Val::Px(44.0),
                width: Val::Px(SLIDER_TRACK_W + SLIDER_PANEL_GUTTER),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::horizontal(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.10, 0.13, 0.85)),
            GlobalZIndex(60),
            TopoTrack,
        ))
        .id();

    commands.spawn((
        Text::new("C"),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::srgb(0.55, 0.57, 0.65)),
        ChildOf(panel),
    ));

    // 88 px visual track bar — `normalize_point` is called against this node
    // in `topo_slider_update` so blend maps to [0, 1] over the visible bar.
    let track_visual = commands
        .spawn((
            Node {
                width: Val::Px(SLIDER_TRACK_W),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.22, 0.23, 0.28)),
            TopoTrackVisual,
            ChildOf(panel),
        ))
        .id();

    // Thumb clamped to [THUMB_HALF, SLIDER_TRACK_W - THUMB_HALF] so it never
    // visually overflows the track bar ends (left offset updated each frame).
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(-2.0),
            left: Val::Px(0.0),
            width: Val::Px(THUMB_HALF * 2.0),
            height: Val::Px(THUMB_HALF * 2.0),
            border_radius: BorderRadius::all(Val::Px(THUMB_HALF)),
            margin: UiRect::left(Val::Px(-THUMB_HALF)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.82, 0.84, 0.92)),
        TopoThumb,
        ChildOf(track_visual),
    ));

    commands.spawn((
        Text::new("P"),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::srgb(0.82, 0.84, 0.92)),
        ChildOf(panel),
    ));
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

/// While the slider panel is pressed, derive blend from cursor X relative to
/// the 88 px visual track bar (`TopoTrackVisual`).  Querying the track bar
/// (not the wider panel) means `normalize_point` maps cursor to [0, 1] over
/// exactly the visible slider range.  `UiGlobalTransform` is Bevy 0.19's UI
/// transform — `GlobalTransform` is 3D-only and absent on UI nodes.
fn topo_slider_update(
    windows: Query<&Window, With<PrimaryWindow>>,
    q_panel: Query<&Interaction, With<TopoTrack>>,
    q_track: Query<(&UiGlobalTransform, &ComputedNode), With<TopoTrackVisual>>,
    mut blend: ResMut<TopoBlend>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    // Only update when the panel (the wide Button) is pressed.
    let pressed = q_panel.iter().any(|i| *i == Interaction::Pressed);
    if !pressed {
        return;
    }
    let Ok((uitf, cnode)) = q_track.single() else {
        return;
    };
    // normalize_point returns None when cursor is outside the node; clamp
    // rather than unwrap so dragging outside the bar still moves the thumb.
    let x = if let Some(norm) = cnode.normalize_point(*uitf, cursor) {
        norm.x
    } else {
        // Cursor left the track while button held: clamp to nearest end.
        let (_, _, translation) = uitf.to_scale_angle_translation();
        if cursor.x < translation.x {
            0.0
        } else {
            1.0
        }
    };
    blend.0 = x.clamp(0.0, 1.0);
}

/// Move the thumb node's `left` offset to match the current blend each frame.
/// Clamped to [THUMB_HALF, SLIDER_TRACK_W - THUMB_HALF] so the thumb circle
/// never overflows the track bar at either extreme.
fn update_topo_thumb(blend: Res<TopoBlend>, mut q_thumb: Query<&mut Node, With<TopoThumb>>) {
    let Ok(mut node) = q_thumb.single_mut() else {
        return;
    };
    let px = (blend.0 * SLIDER_TRACK_W).clamp(THUMB_HALF, SLIDER_TRACK_W - THUMB_HALF);
    node.left = Val::Px(px);
}

/// World-space position for a cell, blended between cubic (blend=0) and
/// pincushion (blend=1).  At blend=1 the middle Y-layer fans out in XZ (1.6×)
/// while top/bottom layers compress gently (0.9×).
///
/// `pub(crate)` so hover.rs can feed the same positions to raycasting.
pub(crate) fn cell_pos(c: Coord, n: u8, blend: f32) -> Vec3 {
    let half = (n as f32 - 1.0) / 2.0;
    let x = c.i as f32 - half;
    let y = c.j as f32 - half;
    let z = c.k as f32 - half;

    if blend <= 0.0 {
        return Vec3::new(x, y, z);
    }

    // Pincushion: middle expands (norm_y=0 → expansion=1.6),
    //             edges compress (norm_y=1 → expansion=0.9).
    let norm_y = if half > 0.0 {
        (y.abs() / half).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let expansion = 1.6 - 0.7 * norm_y;
    Vec3::new(
        x + blend * (x * expansion - x),
        y,
        z + blend * (z * expansion - z),
    )
}

/// Extra Y offset for layer-slicing: layer `j` smoothly lifts as `anim`
/// passes through its threshold.  Layer j=n-1 lifts first (at anim=1), then
/// j=n-2 (anim=2), …, j=1 (anim=4).  Layer j=0 never lifts.
/// `pub(crate)` so hover.rs can offset its raycast centers to match visual positions.
pub(crate) fn lift_y(j: u8, n: u8, anim: f32) -> f32 {
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
    blend: Res<TopoBlend>,
    mut q: Query<(&LatticeCell, Option<&CoreCell>, &mut Transform)>,
) {
    let n = board.0.board.n;
    for (cell, core, mut tf) in &mut q {
        let c = board.0.board.coord(cell.0);
        let mut pos = cell_pos(c, n, blend.0) * peer.0;
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
    blend: Res<TopoBlend>,
    mut q: Query<(&PipeEnds, &PipeBirth, &mut Transform)>,
) {
    let n = board.0.board.n;
    let now = time.elapsed_secs();
    for (ends, birth, mut tf) in &mut q {
        let ca = board.0.board.coord(ends.a);
        let cb = board.0.board.coord(ends.b);
        let mut ea = cell_pos(ca, n, blend.0) * peer.0;
        let mut eb = cell_pos(cb, n, blend.0) * peer.0;
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
