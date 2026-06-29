//! Topology DIAL (a load-bearing exploration mechanic): re-embeds the *same*
//! play state — the same cells, the same connections — into a continuous family
//! of shapes, so the player can rotate the board into whichever portrayal makes
//! the full state easiest to read (interior cells come to the surface).
//!
//! The lattice topology (which cells are adjacent) never changes; only the
//! *embedding* — the map from lattice coord `(i,j,k)` to a 3D position — does.
//! A continuous angular **dial** walks a palindrome ring of stops
//! `Rhombus → Sphere → Cylinder → Torus → Möbius → … → Rhombus`, smoothstep-blended,
//! ordered by how much each embedding glues the boundary to itself so adjacent
//! stops differ minimally. Closure is exact (the ends are the same embedding).
//! A real **R⁴ isoclinic rotation** (projected to R³, phased to identity at every
//! stop) rides on top as the literal "rotate through a higher dimension" feel.
//! Because each tube is drawn endpoint-to-endpoint, any continuous embedding keeps
//! every tube connected. See `docs/analysis/TOPOLOGY_DIAL.md`.

use std::f32::consts::TAU;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::effects::{CoreCell, PipeBirth, PipeEnds, PIPE_GROW_SECS, PIPE_TOTAL_LEN};
use crate::ui_theme as theme;
use crate::BoardResource;
use ciris_game_engine_core::Coord;

/// The embedding keyframes the dial blends between.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Rhombus,
    Sphere,
    Cylinder,
    Torus,
    Mobius,
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Shape::Rhombus => "Rhombus",
            Shape::Sphere => "Sphere",
            Shape::Cylinder => "Cylinder",
            Shape::Torus => "Torus",
            Shape::Mobius => "M\u{f6}bius",
        }
    }
}

/// Palindrome ring of stops over the full 360° dial: out
/// `Rhombus→Sphere→Cylinder→Torus→Möbius`, back `→Torus→Cylinder→Sphere→(Rhombus)`.
/// Eight 45° segments; the wrap 7→0 is `Sphere→Rhombus`, so closure is exact.
const STOPS: [Shape; 8] = [
    Shape::Rhombus,
    Shape::Sphere,
    Shape::Cylinder,
    Shape::Torus,
    Shape::Mobius,
    Shape::Torus,
    Shape::Cylinder,
    Shape::Sphere,
];

/// The dial position, in radians `[0, TAU)`. 0 = Rhombus.
#[derive(Resource, Default)]
pub(crate) struct Topology {
    pub dial: f32,
}

/// Half-extent of the rhombus embedding (matches `render::cell_world_pos`).
const SCALE: f32 = 2.0;
/// How far cells swing through the 4th axis during a morph (the isoclinic R⁴
/// flourish). 0 = flat shape-blend only; ~SCALE = a strong four-dimensional swing.
const L_LIFT: f32 = 1.2;

// ── dial widget geometry (logical px; the dial sits at a fixed screen spot) ──
const DIAL_POS: f32 = 16.0;
const DIAL_SIZE: f32 = 90.0;
const DIAL_R: f32 = 36.0; // indicator orbit radius inside the dial
const DIAL_DOT: f32 = 12.0;
const STOP_DOT: f32 = 14.0; // clickable jump-to-stop markers on the rim
/// Dial centre in logical window pixels (top-left origin).
const DIAL_CX: f32 = DIAL_POS + DIAL_SIZE * 0.5;
const DIAL_CY: f32 = DIAL_POS + DIAL_SIZE * 0.5;

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

/// Global scale multiplier for the marbles — shell, core and ring together (the
/// "marble size" knob). Cores fold it into their breathing scale; everything
/// else (shell / empty marker / ring) takes it directly.
#[derive(Resource)]
pub(crate) struct MarbleSize(pub f32);

impl Default for MarbleSize {
    fn default() -> Self {
        MarbleSize(1.0)
    }
}

/// True while the user is dragging the dial (so the camera orbit is suspended).
#[derive(Resource, Default)]
struct DialDrag(bool);

/// Tags a per-cell entity (frame / core / ring) with its board index so the
/// embedding can place it.
#[derive(Component)]
pub(crate) struct LatticeCell(pub usize);

#[derive(Component)]
struct DialWidget;

#[derive(Component)]
struct DialIndicator;

#[derive(Component)]
struct DialLabel;

/// A clickable jump-to-stop marker on the dial rim, carrying its stop angle.
#[derive(Component)]
struct DialStop(f32);

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Topology>()
        .init_resource::<PeerDistance>()
        .init_resource::<TubeWidth>()
        .init_resource::<MarbleSize>()
        .init_resource::<DialDrag>()
        .add_systems(Startup, spawn_dial)
        .add_systems(Update, dial_stops)
        // Position AFTER sync_effects so a freshly-rebuilt pipe is re-fitted to
        // the embedded cell positions the *same* frame.
        .add_systems(
            Update,
            (
                dial_input,
                update_dial_visual,
                position_cells,
                position_pipes,
            )
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

/// Embed a single cell in a single keyframe shape. All shapes are normalized to
/// keep nearest-neighbour spacing ≳ a marble diameter (see TOPOLOGY_DIAL §5), so
/// cells don't overlap.
fn embed_one(c: Coord, n: u8, s: Shape) -> Vec3 {
    let p = norm(c, n);
    match s {
        Shape::Rhombus => {
            let fcc = Vec3::new(p.y + p.z, p.x + p.z, p.x + p.y);
            fcc * SCALE
        }
        // Round the nested rhombus-shells into nested sphere-shells (a ball): every
        // point keeps its Chebyshev radius but moves onto its own direction.
        Shape::Sphere => {
            let cheb = p.x.abs().max(p.y.abs()).max(p.z.abs());
            let e = p.length();
            let f = if e > 1.0e-4 { cheb / e } else { 1.0 };
            p * f * SCALE
        }
        // i → wrap angle (with a seam), j → height, k → nested radius. The
        // bridge between sphere and torus so the wrap isn't a sudden crush.
        Shape::Cylinder => {
            let theta = (p.x * 0.5 + 0.5) * TAU * 0.85;
            let rr = 1.8 + (p.z * 0.5 + 0.5) * 1.4;
            let h = p.y * 2.6;
            Vec3::new(rr * theta.cos(), h, rr * theta.sin())
        }
        // i → major angle, j → minor angle, k → tube radius (nested tubes). Both
        // angles leave a 0.85 seam so the first and last layer don't coincide;
        // big radius + wide tube keep cells from overlapping (TOPOLOGY_DIAL §5).
        Shape::Torus => {
            let theta = (p.x * 0.5 + 0.5) * TAU * 0.85;
            let phi = (p.y * 0.5 + 0.5) * TAU * 0.85;
            let rr = 0.8 + (p.z * 0.5 + 0.5) * 1.8;
            let big = 3.2;
            Vec3::new(
                (big + rr * phi.cos()) * theta.cos(),
                rr * phi.sin(),
                (big + rr * phi.cos()) * theta.sin(),
            )
        }
        // A proper Möbius band: i runs the full loop; the WIDTH axis (j) rotates
        // by half the loop angle so it flips by the time it returns. Thickness (k)
        // factor 0.84 so the stacked layers don't overlap (TOPOLOGY_DIAL §5).
        Shape::Mobius => {
            let f = p.x * 0.5 + 0.5;
            let l = f * TAU;
            let half = l * 0.5;
            let radial = Vec3::new(l.cos(), 0.0, l.sin());
            let up = Vec3::Y;
            let across = radial * half.cos() + up * half.sin();
            let normal = radial * (-half.sin()) + up * half.cos();
            radial * 3.2 + across * (p.y * 2.2) + normal * (p.z * 0.84)
        }
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// An isoclinic R⁴ rotation of a 4D lift of the point, projected back to R³ by
/// dropping `w`. Phased so the angle is `s·TAU` → identity at `s = 0` and `s = 1`,
/// i.e. every stop is shown clean and continuity holds across stop boundaries.
/// This is the literal "rotate through a higher dimension" (TOPOLOGY_DIAL §4).
fn rot4_flourish(p: Vec3, np: Vec3, s: f32) -> Vec3 {
    // Outer shells swing furthest through the 4th axis; centred so the whole body
    // rotates, not just one side.
    let cheb = np.x.abs().max(np.y.abs()).max(np.z.abs());
    let w = (cheb - 0.5) * 2.0 * L_LIFT;
    let a = s * TAU;
    let (sa, ca) = a.sin_cos();
    Vec3::new(
        p.x * ca - w * sa,   // (x, w) plane
        p.y * ca - p.z * sa, // (y, z) plane
        p.y * sa + p.z * ca,
    ) // w' = p.x*sa + w*ca is dropped (orthographic R⁴→R³)
}

/// Embed a cell at the current dial angle: blend the two bracketing keyframes
/// (smoothstep), then apply the 4D flourish. Continuous in `dial`, so every tube
/// stays connected.
pub(crate) fn embed(c: Coord, n: u8, topo: &Topology) -> Vec3 {
    let k = STOPS.len() as f32;
    let x = topo.dial.rem_euclid(TAU) / TAU * k;
    let i0 = (x.floor() as usize) % STOPS.len();
    let i1 = (i0 + 1) % STOPS.len();
    let s = smooth(x - x.floor());
    let base = embed_one(c, n, STOPS[i0]).lerp(embed_one(c, n, STOPS[i1]), s);
    rot4_flourish(base, norm(c, n), s)
}

/// Start/continue/end a dial drag, suspend camera orbit while dragging, and set
/// the dial angle from the cursor's bearing around the dial centre.
fn dial_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    interactions: Query<&Interaction, With<DialWidget>>,
    mut drag: ResMut<DialDrag>,
    mut topo: ResMut<Topology>,
    mut cam: Query<&mut PanOrbitCamera, With<crate::render::MainCam>>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        drag.0 = true;
    }
    if mouse.just_released(MouseButton::Left) {
        drag.0 = false;
    }
    // Don't let the orbit camera spin while the dial is being dragged.
    if let Ok(mut c) = cam.single_mut() {
        c.enabled = !drag.0;
    }
    if drag.0 {
        if let Ok(win) = windows.single() {
            if let Some(cur) = win.cursor_position() {
                let dx = cur.x - DIAL_CX;
                let dy = cur.y - DIAL_CY;
                // 0 = up (12 o'clock), increasing clockwise.
                topo.dial = dx.atan2(-dy).rem_euclid(TAU);
            }
        }
    }
}

/// Click a rim marker to jump the dial straight to that stop's shape.
fn dial_stops(
    q: Query<(&Interaction, &DialStop), Changed<Interaction>>,
    mut topo: ResMut<Topology>,
) {
    for (interaction, stop) in &q {
        if *interaction == Interaction::Pressed {
            topo.dial = stop.0;
        }
    }
}

/// Move the indicator dot around the rim and update the stop-name label.
fn update_dial_visual(
    topo: Res<Topology>,
    mut indicator: Query<&mut Node, With<DialIndicator>>,
    mut label: Query<&mut Text, With<DialLabel>>,
) {
    let (s, c) = topo.dial.sin_cos();
    if let Ok(mut node) = indicator.single_mut() {
        node.left = Val::Px(DIAL_SIZE * 0.5 + DIAL_R * s - DIAL_DOT * 0.5);
        node.top = Val::Px(DIAL_SIZE * 0.5 - DIAL_R * c - DIAL_DOT * 0.5);
    }
    let x = topo.dial.rem_euclid(TAU) / TAU * STOPS.len() as f32;
    let nearest = (x.round() as usize) % STOPS.len();
    if let Ok(mut t) = label.single_mut() {
        *t = Text::new(STOPS[nearest].name());
    }
}

/// Place every per-cell entity at its embedded position each frame.
fn position_cells(
    board: Res<BoardResource>,
    topo: Res<Topology>,
    peer: Res<PeerDistance>,
    marble: Res<MarbleSize>,
    mut q: Query<(&LatticeCell, Option<&CoreCell>, &mut Transform)>,
) {
    let n = board.0.board.n;
    for (cell, core, mut tf) in &mut q {
        tf.translation = embed(board.0.board.coord(cell.0), n, &topo) * peer.0;
        // Cores fold marble size into their own breathing scale (breathe_cores);
        // shell / empty marker / ring take it directly here.
        if core.is_none() {
            tf.scale = Vec3::splat(marble.0);
        }
    }
}

/// Re-fit every straight tube between its two cells' current embedded positions
/// (so the tubes stay connected through any dial angle), carrying the §4.6 grow-in.
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
        let dir = eb - ea;
        let len = dir.length().max(1.0e-4);
        let grow = smooth(((now - birth.0) / PIPE_GROW_SECS).clamp(0.0, 1.0)).max(0.02);
        tf.translation = (ea + eb) * 0.5;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir / len);
        // Divide by the capsule's TRUE length so the tube spans exactly centre-
        // to-centre (ends buried inside both spheres) instead of spiking past.
        tf.scale = Vec3::new(tube.0, (len / PIPE_TOTAL_LEN) * grow, tube.0);
    }
}

/// Top-left rotary dial that sweeps the embedding family.
fn spawn_dial(mut commands: Commands, topo: Res<Topology>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(DIAL_POS),
                left: Val::Px(DIAL_POS),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },
            GlobalZIndex(60),
        ))
        .id();
    let dial = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(DIAL_SIZE),
                height: Val::Px(DIAL_SIZE),
                border_radius: BorderRadius::all(Val::Px(DIAL_SIZE * 0.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.10, 0.13, 0.85)),
            DialWidget,
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(DIAL_DOT),
            height: Val::Px(DIAL_DOT),
            left: Val::Px(DIAL_SIZE * 0.5 - DIAL_DOT * 0.5),
            top: Val::Px(DIAL_SIZE * 0.5 - DIAL_R - DIAL_DOT * 0.5),
            border_radius: BorderRadius::all(Val::Px(DIAL_DOT * 0.5)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.95, 0.97, 1.0)),
        DialIndicator,
        ChildOf(dial),
    ));
    // Clickable jump-to-stop markers, one per stop angle around the rim.
    for i in 0..STOPS.len() {
        let ang = (i as f32 / STOPS.len() as f32) * TAU;
        let (s, c) = ang.sin_cos();
        commands.spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(STOP_DOT),
                height: Val::Px(STOP_DOT),
                left: Val::Px(DIAL_SIZE * 0.5 + DIAL_R * s - STOP_DOT * 0.5),
                top: Val::Px(DIAL_SIZE * 0.5 - DIAL_R * c - STOP_DOT * 0.5),
                border_radius: BorderRadius::all(Val::Px(STOP_DOT * 0.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.6, 0.62, 0.7, 0.55)),
            DialStop(ang),
            ChildOf(dial),
        ));
    }
    let spec = theme::BtnSpec::filled();
    let x = topo.dial.rem_euclid(TAU) / TAU * STOPS.len() as f32;
    let nearest = (x.round() as usize) % STOPS.len();
    let label = theme::text(
        &mut commands,
        root,
        STOPS[nearest].name(),
        theme::font(theme::DISPLAY, theme::SIZE_SM, FontWeight::MEDIUM),
        spec.text,
    );
    commands.entity(label).insert(DialLabel);
}
