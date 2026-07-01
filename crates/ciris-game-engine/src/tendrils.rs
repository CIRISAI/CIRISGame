//! Plasma TENDRILS: soft flowing tendrils of light along every valid bond
//! position — wherever a tube could validly form (an adjacency between two
//! placeable cells). A gentler hint than the old peer-spikes: it still reveals
//! the lattice connectivity in any embedding, but reads as a calm web of light
//! that brightens and rushes toward the cursor (the plasma `hover` cue).

use bevy::prelude::*;

use crate::hover::{HoverState, HoveredCell};
use crate::plasma::PlasmaMaterial;
use crate::render::BLOOM_LAYER;
use crate::topology::{cell_pos, PeerDistance};
use crate::BoardResource;
use ciris_game_engine_core::{Board, CellState};

/// Tendril radius (thin filament).
const TENDRIL_R: f32 = 0.02;

/// One bond's two cell indices plus its eased glow (0 hidden ‥ 1 full), so the
/// tendril fades in/out smoothly as the cursor moves on/off its cells.
#[derive(Component)]
struct TendrilEdge {
    a: usize,
    b: usize,
    glow: f32,
}

/// Fade time-constant (s) for the tendril glow.
const FADE_TAU: f32 = 0.10;

/// The shared plasma material, so the cursor focus can drive its `hover` uniform.
#[derive(Resource)]
struct TendrilMat(Handle<PlasmaMaterial>);

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (position_tendrils, tendril_hover).after(crate::effects::sync_effects),
    );
}

/// Spawn one tendril per unique in-bounds bond, hidden until placed. Called from
/// `render::setup`.
pub(crate) fn setup_tendrils(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    plasma_mats: &mut Assets<PlasmaMaterial>,
    board: &Board,
) {
    let mesh = meshes.add(Cylinder::new(TENDRIL_R, 1.0));
    // A faint resting web that the cursor lights up (lower opacity / floor / glow
    // than the old ghost cage so the full lattice reads as a calm haze).
    let mat = plasma_mats.add(PlasmaMaterial {
        tint: LinearRgba::new(0.34, 0.58, 0.84, 0.22),
        params: Vec4::new(0.55, 1.25, 0.08, 1.5),
        hover: Vec4::ZERO,
    });
    commands.insert_resource(TendrilMat(mat.clone()));
    for cell in 0..board.len() {
        for nb in board.neighbors(cell) {
            if nb <= cell {
                continue; // each bond once
            }
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::default(),
                Visibility::Hidden,
                bevy::camera::visibility::RenderLayers::from_layers(&[0, BLOOM_LAYER]),
                TendrilEdge {
                    a: cell,
                    b: nb,
                    glow: 0.0,
                },
            ));
        }
    }
}

/// Stretch the hovered cell's tendrils between its cells' embedded positions.
/// Tendrils are a mouse-over hint only: a bond shows just when one of its cells is
/// the cell under the cursor (and both endpoints are placeable — a valid tube
/// location). Everything else stays hidden.
fn position_tendrils(
    time: Res<Time>,
    board: Res<BoardResource>,
    peer: Res<PeerDistance>,
    blend: Res<crate::topology::TopoBlend>,
    hovered: Res<HoveredCell>,
    mut q: Query<(&mut TendrilEdge, &mut Transform, &mut Visibility)>,
) {
    let n = board.0.board.n;
    let placeable = |idx: usize| {
        matches!(
            board.0.board.get(idx),
            CellState::Live(_) | CellState::Empty
        )
    };
    let k = (1.0 - (-time.delta_secs() / FADE_TAU).exp()).clamp(0.0, 1.0);
    for (mut edge, mut tf, mut vis) in &mut q {
        let on = (hovered.0 == Some(edge.a) || hovered.0 == Some(edge.b))
            && placeable(edge.a)
            && placeable(edge.b);
        // Ease the glow toward on/off so it fades in/out as the cursor moves.
        edge.glow += ((if on { 1.0 } else { 0.0 }) - edge.glow) * k;
        if edge.glow < 0.02 {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        let ea = cell_pos(board.0.board.coord(edge.a), n, blend.0) * peer.0;
        let eb = cell_pos(board.0.board.coord(edge.b), n, blend.0) * peer.0;
        let dir = eb - ea;
        let len = dir.length().max(1.0e-4);
        tf.translation = (ea + eb) * 0.5;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir / len);
        // Radius grows with the eased glow → a smooth fade in/out.
        tf.scale = Vec3::new(edge.glow, len, edge.glow);
        *vis = Visibility::Visible;
    }
}

/// Drive the tendril plasma's `hover` uniform from the cursor focus, so the web
/// rushes/brightens toward the cell being selected.
fn tendril_hover(
    hover: Res<HoverState>,
    mat: Option<Res<TendrilMat>>,
    mut mats: ResMut<Assets<PlasmaMaterial>>,
) {
    let Some(mat) = mat else {
        return;
    };
    if let Some(mut m) = mats.get_mut(&mat.0) {
        m.hover = hover.focus();
    }
}
