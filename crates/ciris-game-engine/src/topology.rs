//! Topology widget (a load-bearing exploration mechanic): re-embeds the *same*
//! play state — the same cells, the same connections — into different shapes so
//! the player can rotate the board into whichever portrayal makes the full state
//! easiest to read (interior cells come to the surface, every face visible).
//!
//! The lattice topology (which cells are adjacent) never changes; only the
//! *embedding* — the map from lattice coord `(i,j,k)` to a 3D position — does.
//! Because each glass tube is drawn endpoint-to-endpoint between its two cells'
//! current positions, ANY continuous coordinate map keeps every tube connected
//! (a true topological transformation). Switching shapes eases (morphs) between
//! embeddings, so you literally watch the cube unfold into a torus / möbius /
//! sphere with the play state intact.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::effects::{PipeBirth, PipeEnds, PIPE_GROW_SECS, PIPE_LEN};
use crate::ui_theme as theme;
use crate::BoardResource;
use ciris_game_engine_core::Coord;

/// The available embeddings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Cube,
    Sphere,
    Torus,
    Mobius,
}

impl Shape {
    const ALL: [Shape; 4] = [Shape::Cube, Shape::Sphere, Shape::Torus, Shape::Mobius];
    fn name(self) -> &'static str {
        match self {
            Shape::Cube => "Cube",
            Shape::Sphere => "Sphere",
            Shape::Torus => "Torus",
            Shape::Mobius => "M\u{f6}bius",
        }
    }
    fn next(self) -> Shape {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Current embedding + an in-flight morph from `from` to `to` (`t` 0→1).
#[derive(Resource)]
pub(crate) struct Topology {
    from: Shape,
    to: Shape,
    t: f32,
}

impl Default for Topology {
    fn default() -> Self {
        Topology {
            from: Shape::Cube,
            to: Shape::Cube,
            t: 1.0,
        }
    }
}

/// Seconds to morph between two shapes.
const MORPH_SECS: f32 = 1.6;
/// Half-extent of the cube embedding (matches `render::cell_world_pos`).
const SCALE: f32 = 2.0;

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

/// Tags a per-cell entity (frame / core / ring) with its board index so the
/// embedding can place it.
#[derive(Component)]
pub(crate) struct LatticeCell(pub usize);

#[derive(Component)]
struct TopoButton;

#[derive(Component)]
struct TopoLabel;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Topology>()
        .init_resource::<PeerDistance>()
        .init_resource::<TubeWidth>()
        .add_systems(Startup, spawn_widget)
        // Position AFTER sync_effects so a freshly-rebuilt pipe is re-fitted to
        // the embedded cell positions the *same* frame — otherwise it flashes at
        // the raw cube location for a frame when a stone is placed.
        .add_systems(
            Update,
            (cycle, advance, position_cells, position_pipes)
                .chain()
                .after(crate::effects::sync_effects),
        );
}

/// Lattice coord → `[-1, 1]³`.
fn norm(c: Coord, n: u8) -> Vec3 {
    let d = (n.max(2) - 1) as f32;
    Vec3::new(
        c.i as f32 / d * 2.0 - 1.0,
        c.j as f32 / d * 2.0 - 1.0,
        c.k as f32 / d * 2.0 - 1.0,
    )
}

/// Embed a single cell in a single shape.
fn embed_one(c: Coord, n: u8, s: Shape) -> Vec3 {
    let p = norm(c, n);
    match s {
        Shape::Cube => p * SCALE,
        // Round the nested cube-shells into nested sphere-shells (a ball): every
        // point keeps its Chebyshev radius but moves onto its own direction.
        Shape::Sphere => {
            let cheb = p.x.abs().max(p.y.abs()).max(p.z.abs());
            let e = p.length();
            let f = if e > 1.0e-4 { cheb / e } else { 1.0 };
            p * f * SCALE
        }
        // i → major angle, j → minor angle, k → tube radius (nested tubes). The
        // 0.85 leaves a seam where the lattice doesn't wrap (no false bonds).
        Shape::Torus => {
            let theta = (p.x * 0.5 + 0.5) * TAU * 0.85;
            let phi = (p.y * 0.5 + 0.5) * TAU;
            let rr = 0.5 + (p.z * 0.5 + 0.5) * 0.9;
            let big = 2.2;
            Vec3::new(
                (big + rr * phi.cos()) * theta.cos(),
                rr * phi.sin(),
                (big + rr * phi.cos()) * theta.sin(),
            )
        }
        // A proper Möbius band. i runs the full loop (0→2π); the WIDTH axis (j)
        // rotates by half the loop angle so it flips by the time it returns — the
        // signature half-twist. The band is wide (j) and very thin (k) so it
        // reads as a twisting ribbon, not a bar.
        Shape::Mobius => {
            let f = p.x * 0.5 + 0.5; // 0..1 around the loop
            let l = f * TAU;
            let half = l * 0.5;
            let radial = Vec3::new(l.cos(), 0.0, l.sin());
            let up = Vec3::Y;
            // Width direction lies in the (radial, up) plane, rotating by `half`.
            let across = radial * half.cos() + up * half.sin();
            let normal = radial * (-half.sin()) + up * half.cos();
            radial * 2.8 + across * (p.y * 2.1) + normal * (p.z * 0.14)
        }
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Embed a cell at the current morph state. During a morph the whole structure
/// also tumbles through a dual-axis rotation that *completes* (returns to
/// aligned) exactly as the morph finishes — so a transition reads as the lattice
/// rotating through a higher dimension and re-settling, not cells sliding in
/// straight lines.
fn embed(c: Coord, n: u8, topo: &Topology) -> Vec3 {
    if topo.t >= 1.0 || topo.from == topo.to {
        return embed_one(c, n, topo.to);
    }
    let s = smooth(topo.t);
    let base = embed_one(c, n, topo.from).lerp(embed_one(c, n, topo.to), s);
    // One full turn on each of two axes → both land on identity at s = 1.
    let tumble = Quat::from_axis_angle(Vec3::Y, s * TAU) * Quat::from_axis_angle(Vec3::X, s * TAU);
    tumble * base
}

fn advance(time: Res<Time>, mut topo: ResMut<Topology>) {
    if topo.t < 1.0 {
        topo.t = (topo.t + time.delta_secs() / MORPH_SECS).min(1.0);
    }
}

/// Cycle to the next shape on a button press and start the morph.
fn cycle(
    q: Query<&Interaction, (Changed<Interaction>, With<TopoButton>)>,
    mut topo: ResMut<Topology>,
    mut label: Query<&mut Text, With<TopoLabel>>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            let current = topo.to;
            topo.from = current;
            topo.to = current.next();
            topo.t = 0.0;
            if let Ok(mut text) = label.single_mut() {
                *text = Text::new(topo.to.name());
            }
        }
    }
}

/// Place every per-cell entity at its embedded position each frame.
fn position_cells(
    board: Res<BoardResource>,
    topo: Res<Topology>,
    peer: Res<PeerDistance>,
    mut q: Query<(&LatticeCell, &mut Transform)>,
) {
    let n = board.0.board.n;
    for (cell, mut tf) in &mut q {
        tf.translation = embed(board.0.board.coord(cell.0), n, &topo) * peer.0;
    }
}

/// Re-fit every pipe between its two cells' current embedded positions (so the
/// tubes stay connected through any morph), carrying the §4.6 grow-in.
/// Per-bond perpendicular offset so the two face-diagonals (which would otherwise
/// cross at the face centre) bow to opposite sides and pass clear of each other —
/// keeping different-colour tubes from crossing. Offset is along the bond's
/// zero-axis (the face normal); the sign comes from the product of its two
/// non-zero components, which is opposite for the two diagonals of any face.
fn crossing_offset(a: Coord, b: Coord) -> Vec3 {
    const E: f32 = 0.32;
    let di = b.i as i32 - a.i as i32;
    let dj = b.j as i32 - a.j as i32;
    let dk = b.k as i32 - a.k as i32;
    let s = |x: i32| -> f32 { (x.signum()) as f32 };
    if dk == 0 {
        Vec3::Z * (s(di * dj) * E)
    } else if dj == 0 {
        Vec3::Y * (s(di * dk) * E)
    } else if di == 0 {
        Vec3::X * (s(dj * dk) * E)
    } else {
        Vec3::ZERO
    }
}

fn position_pipes(
    time: Res<Time>,
    board: Res<BoardResource>,
    topo: Res<Topology>,
    peer: Res<PeerDistance>,
    tube: Res<TubeWidth>,
    mut q: Query<(&PipeEnds, &PipeBirth, &mut Transform)>,
) {
    let n = board.0.board.n;
    let now = time.elapsed_secs();
    for (ends, birth, mut tf) in &mut q {
        let ca = board.0.board.coord(ends.a);
        let cb = board.0.board.coord(ends.b);
        let ea = embed(ca, n, &topo) * peer.0;
        let eb = embed(cb, n, &topo) * peer.0;
        // Bow the midpoint perpendicular (crossings → opposite bows); each half
        // runs from a sphere centre to that bowed midpoint, so the tube curves
        // out and back into the sphere instead of crossing through the centre.
        let mid = (ea + eb) * 0.5 + crossing_offset(ca, cb);
        let (start, end) = if ends.half == 0 { (ea, mid) } else { (mid, eb) };
        let dir = end - start;
        let len = dir.length().max(1.0e-4);
        let grow = smooth(((now - birth.0) / PIPE_GROW_SECS).clamp(0.0, 1.0)).max(0.02);
        tf.translation = (start + end) * 0.5;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir / len);
        tf.scale = Vec3::new(tube.0, (len / PIPE_LEN) * grow, tube.0);
    }
}

/// Top-left button that cycles the topology.
fn spawn_widget(mut commands: Commands, topo: Res<Topology>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            GlobalZIndex(60),
        ))
        .id();
    let spec = theme::BtnSpec::filled();
    let btn = commands
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(spec.colors.normal),
            spec.colors,
            TopoButton,
            ChildOf(root),
        ))
        .id();
    let label = theme::text(
        &mut commands,
        btn,
        topo.to.name(),
        theme::font(theme::DISPLAY, theme::SIZE_SM, FontWeight::MEDIUM),
        spec.text,
    );
    commands.entity(label).insert(TopoLabel);
}
