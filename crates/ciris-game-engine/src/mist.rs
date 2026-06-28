//! Dead-group volumetric mist (DESIGN_BRIEF §3.6) and the dispersal cross-fade
//! (§4.6). A custom [`MistMaterial`] (an `AsBindGroup` fragment-raymarch material;
//! shader in `assets/shaders/mist.wgsl`) is drawn on a per-cell sphere that sits
//! inside the glass shell. Every cell owns its own mist entity *and its own
//! material instance* so the §4.6 cross-fade can animate one crater cell at a time
//! without touching its neighbours.
//!
//! Driven entirely from `GameState`: `render::sync_board` diffs the board on every
//! [`BoardDirty`] and calls [`MistState::on_transition`] to push each cell into a
//! [`MistPhase`]; [`animate_mist`] then advances those phases against `Time` and
//! rewrites the per-cell material uniforms. Nothing here decides game state.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::palette;
use ciris_game_engine_core::CellState;

/// Mist sphere radius — fills most of the glass shell but stays inside it so the
/// shell still reads as a lens around the smoke (§3.6).
const MIST_RADIUS: f32 = 0.36;
/// Analytic raymarch radius, a hair under the proxy mesh so the mesh always
/// covers the sphere's screen footprint.
const MIST_R_ANALYTIC: f32 = MIST_RADIUS * 0.98;
/// Noise frequency (§3.6 freq 1.4), scaled to the small cell radius so the fbm
/// shows a couple of churning lobes rather than one flat blob.
const MIST_FREQ: f32 = 4.2;

/// Black temp-dead flow speed, units/s (§3.6).
const FLOW_BLACK: f32 = 0.6;
/// Green perma-dead flow speed, units/s — slower, at rest (§3.6).
const FLOW_GREEN: f32 = 0.3;

/// §4.6 timings.
const APPEAR_SECS: f32 = 0.4;
const CROSSFADE_SECS: f32 = 0.8;
const FADEOUT_SECS: f32 = 0.6;

/// Black mist tint (linear). A dark smoke grey — visibly above pure black so it
/// reads as churning volume through the dimmed shell rather than a void.
fn mist_black() -> LinearRgba {
    LinearRgba::rgb(0.05, 0.05, 0.06)
}

/// Green perma-dead mist tint (linear) — Verdigris, lifted slightly so the wisps
/// glow against the Bone backdrop.
fn mist_green() -> LinearRgba {
    palette::STEWARD_VERDIGRIS_LINEAR.to_linear() * 1.3
}

/// The per-cell mist raymarch material (DESIGN_BRIEF §3.6). One instance per cell.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct MistMaterial {
    /// rgb = tint, a = appear/fade factor in `[0, 1]` (scales density, §4.6).
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = flow speed, y = noise freq, z = sphere radius, w = unused.
    #[uniform(1)]
    pub flow: Vec4,
    /// xyz = cell world centre, w = unused.
    #[uniform(2)]
    pub center: Vec4,
}

impl Material for MistMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/mist.wgsl".into()
    }

    // Opaque + discard-on-density, rendered before Transmissive3d so the glass
    // shell refracts the smoke (§3.6).
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

/// What a cell's mist is doing right now. Phase *transitions* are set by the
/// board diff (`render::sync_board`); phase *progress* is advanced per frame by
/// [`animate_mist`].
#[derive(Clone, Copy, PartialEq)]
enum MistPhase {
    /// No mist; entity hidden.
    Hidden,
    /// Collapse: black mist materialises, density 0 → 1 over [`APPEAR_SECS`].
    Appear,
    /// Steady black temp-dead mist.
    Black,
    /// Dispersal: black → green cross-fade over [`CROSSFADE_SECS`] (§4.6).
    ToGreen,
    /// Steady green perma-dead mist, at rest.
    Green,
    /// Dispersal rebuild: mist dissolves, density 1 → 0 over [`FADEOUT_SECS`].
    FadeOut,
}

#[derive(Clone, Copy)]
struct MistFx {
    phase: MistPhase,
    /// `Time::elapsed_secs` at the start of the current phase.
    start: f32,
}

/// Per-cell mist state plus the per-cell material handles, indexed by linear cell.
#[derive(Resource)]
pub(crate) struct MistState {
    fx: Vec<MistFx>,
    handles: Vec<Handle<MistMaterial>>,
}

/// Tags a mist entity with its linear cell index.
#[derive(Component)]
pub(crate) struct MistCell(usize);

impl MistState {
    /// Drive cell `idx` into the right phase for a `prev → next` board change.
    /// Called once per changed cell from the board diff (§4.6). `now` is
    /// `Time::elapsed_secs`. No-op transitions are filtered by the caller.
    pub(crate) fn on_transition(&mut self, idx: usize, prev: CellState, next: CellState, now: f32) {
        let phase = match (prev, next) {
            (CellState::Live(_), CellState::TempDead(_)) => MistPhase::Appear,
            (CellState::TempDead(_), CellState::PermaDead) => MistPhase::ToGreen,
            (CellState::TempDead(_), CellState::Live(_)) => MistPhase::FadeOut,
            (CellState::TempDead(_), CellState::Empty) => MistPhase::FadeOut,
            // Pre-placed / fallback perma-dead appears already at rest.
            (_, CellState::PermaDead) => MistPhase::Green,
            // Fallback collapse into smoke.
            (_, CellState::TempDead(_)) => MistPhase::Appear,
            // Anything live or empty carries no mist.
            (_, CellState::Live(_)) | (_, CellState::Empty) => MistPhase::Hidden,
        };
        self.fx[idx] = MistFx { phase, start: now };
    }

    /// Snap cell `idx` to its resting phase for the given state, no animation.
    /// Used when a fresh game is initialised (turn 0).
    pub(crate) fn reset_cell(&mut self, idx: usize, state: CellState) {
        let phase = match state {
            CellState::PermaDead => MistPhase::Green,
            CellState::TempDead(_) => MistPhase::Black,
            _ => MistPhase::Hidden,
        };
        self.fx[idx] = MistFx { phase, start: 0.0 };
    }
}

/// Register the mist material plugin. Added from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<MistMaterial>::default());
}

/// Spawn one hidden mist entity + material per cell. Called from `render::setup`.
pub(crate) fn setup_mist(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mist_materials: &mut Assets<MistMaterial>,
    centers: &[Vec3],
) {
    let mesh = meshes.add(Sphere::new(MIST_RADIUS).mesh().ico(4).unwrap());
    let count = centers.len();
    let mut handles = Vec::with_capacity(count);
    for &center in centers {
        let handle = mist_materials.add(MistMaterial {
            color: LinearRgba::new(0.0, 0.0, 0.0, 0.0),
            flow: Vec4::new(FLOW_BLACK, MIST_FREQ, MIST_R_ANALYTIC, 0.0),
            center: center.extend(0.0),
        });
        let idx = handles.len();
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(handle.clone()),
            Transform::from_translation(center),
            Visibility::Hidden,
            MistCell(idx),
        ));
        handles.push(handle);
    }

    commands.insert_resource(MistState {
        fx: vec![
            MistFx {
                phase: MistPhase::Hidden,
                start: 0.0,
            };
            count
        ],
        handles,
    });
}

/// Advance every cell's mist phase and rewrite its material uniforms each frame
/// (DESIGN_BRIEF §3.6 flow + §4.6 cross-fade). Reads no game state.
pub(crate) fn animate_mist(
    time: Res<Time>,
    mut state: ResMut<MistState>,
    mut materials: ResMut<Assets<MistMaterial>>,
    mut cells: Query<(&MistCell, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    for (cell, mut vis) in &mut cells {
        let idx = cell.0;
        let fx = state.fx[idx];
        let e = (t - fx.start).max(0.0);

        // Resolve the visible parameters for this phase, advancing finished
        // timed phases into their resting successor.
        let (visible, color, flow_speed, next) = match fx.phase {
            MistPhase::Hidden => (false, LinearRgba::new(0.0, 0.0, 0.0, 0.0), FLOW_BLACK, None),
            MistPhase::Appear => {
                let p = (e / APPEAR_SECS).clamp(0.0, 1.0);
                let c = mist_black().with_alpha(p);
                let done = if p >= 1.0 {
                    Some(MistPhase::Black)
                } else {
                    None
                };
                (true, c, FLOW_BLACK, done)
            }
            MistPhase::Black => (true, mist_black().with_alpha(1.0), FLOW_BLACK, None),
            MistPhase::ToGreen => {
                let p = (e / CROSSFADE_SECS).clamp(0.0, 1.0);
                let c = lerp_rgba(mist_black(), mist_green(), p).with_alpha(1.0);
                let flow = FLOW_BLACK + (FLOW_GREEN - FLOW_BLACK) * p;
                let done = if p >= 1.0 {
                    Some(MistPhase::Green)
                } else {
                    None
                };
                (true, c, flow, done)
            }
            MistPhase::Green => (true, mist_green().with_alpha(1.0), FLOW_GREEN, None),
            MistPhase::FadeOut => {
                let p = (e / FADEOUT_SECS).clamp(0.0, 1.0);
                let c = mist_black().with_alpha(1.0 - p);
                let done = if p >= 1.0 {
                    Some(MistPhase::Hidden)
                } else {
                    None
                };
                (p < 1.0, c, FLOW_BLACK, done)
            }
        };

        if let Some(phase) = next {
            state.fx[idx] = MistFx { phase, start: t };
        }

        let want = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }

        if visible {
            if let Some(mut mat) = materials.get_mut(&state.handles[idx]) {
                mat.color = color;
                mat.flow.x = flow_speed;
            }
        }
    }
}

/// Linear-space lerp between two colours (alpha ignored; set by the caller).
fn lerp_rgba(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba::rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}
