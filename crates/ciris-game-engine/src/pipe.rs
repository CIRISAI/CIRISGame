//! Liquid-pigment pipe material (DESIGN_BRIEF §3.4): the steward pigment flows as
//! a liquid through the glass channel between connected same-colour cells, pooling
//! toward world-down (gravity) and sloshing when the camera orbits.
//!
//! Like [`crate::mist`] this is a custom webgl2-safe [`AsBindGroup`] fragment
//! material (shader `assets/shaders/pipe.wgsl`, bind group 3): fragment-only,
//! constant loop bounds, vec4-aligned uniforms, validated through naga to GLSL ES
//! 300. Each pipe owns its own material instance so the gravity fill can be
//! measured against that pipe's own world centre and extent; a single global
//! [`SloshState`] spring is pushed into every instance each frame so the whole
//! lattice's liquid sloshes together as the camera moves.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::palette;
use ciris_game_engine_core::Steward;

/// Liquid opacity (alpha of the pigment column) — translucent so it still reads as
/// liquid inside the glass channel rather than a solid rod.
const LIQUID_ALPHA: f32 = 0.9;
/// Resting fill fraction of the tube (0 empty .. 1 full).
const FILL: f32 = 0.72;
/// Flow scroll speed (units/s) of the liquid noise.
const FLOW_SPEED: f32 = 0.5;
/// Liquid noise frequency (churning lobes per unit).
const FLOW_FREQ: f32 = 9.0;
/// How strongly the slosh displacement tilts the liquid surface.
const SLOSH_STRENGTH: f32 = 1.0;

/// Slosh spring stiffness (pulls the surface back to world-level).
const SLOSH_K: f32 = 55.0;
/// Slosh spring damping (settles the oscillation).
const SLOSH_DAMP: f32 = 7.0;
/// Slosh impulse injected per rad/s of camera angular velocity.
const SLOSH_GAIN: f32 = 0.04;
/// Clamp on the slosh displacement magnitude so a fast orbit can't invert the fill.
const SLOSH_MAX: f32 = 0.6;

/// The per-pipe liquid material (DESIGN_BRIEF §3.4). One instance per pipe.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PipeMaterial {
    /// rgb = steward pigment (linear), a = liquid opacity.
    #[uniform(0)]
    pub color: LinearRgba,
    /// xyz = pipe world centre, w = vertical half-extent (along world up).
    #[uniform(1)]
    pub geom: Vec4,
    /// x = fill, y = flow speed, z = noise freq, w = slosh strength.
    #[uniform(2)]
    pub dynamics: Vec4,
    /// xyz = slosh displacement (world, lateral), w = unused.
    #[uniform(3)]
    pub slosh: Vec4,
}

impl Material for PipeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/pipe.wgsl".into()
    }

    // Blend so the carved-away air gap above the surface shows the channel through,
    // and the liquid reads as translucent pigment.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Build a pipe's liquid material: `center`/`half_extent` describe the tube's
/// world placement so the shader can pool the fill toward world-down for it.
pub(crate) fn material(steward: Steward, center: Vec3, half_extent: f32) -> PipeMaterial {
    let slot = steward.slot() as usize;
    let rgba = palette::STEWARD_LINEAR[slot].to_linear();
    PipeMaterial {
        color: LinearRgba::new(rgba.red, rgba.green, rgba.blue, LIQUID_ALPHA),
        geom: center.extend(half_extent.max(1.0e-3)),
        dynamics: Vec4::new(FILL, FLOW_SPEED, FLOW_FREQ, SLOSH_STRENGTH),
        slosh: Vec4::ZERO,
    }
}

/// Global slosh spring, integrated from the camera's angular velocity and pushed
/// into every pipe material each frame.
#[derive(Resource, Default)]
struct SloshState {
    /// Current surface-tilt displacement (world, lateral).
    disp: Vec3,
    /// Spring velocity.
    vel: Vec3,
    /// Previous camera (yaw, pitch); `None` until the first frame.
    last: Option<Vec2>,
}

/// Register the pipe material plugin and the slosh driver. Added from
/// `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PipeMaterial>::default())
        .init_resource::<SloshState>()
        .add_systems(Update, drive_slosh);
}

/// Track the PanOrbitCamera's angular velocity, advance the damped slosh spring,
/// and write the resulting surface tilt into every live pipe material. Reads no
/// game state.
fn drive_slosh(
    time: Res<Time>,
    cameras: Query<&PanOrbitCamera>,
    mut slosh: ResMut<SloshState>,
    mut materials: ResMut<Assets<PipeMaterial>>,
) {
    let dt = time.delta_secs().max(1.0e-4);

    // Camera angular velocity from the yaw/pitch change since last frame.
    let mut omega = Vec2::ZERO;
    if let Ok(cam) = cameras.single() {
        let cur = Vec2::new(cam.yaw.unwrap_or(0.0), cam.pitch.unwrap_or(0.0));
        if let Some(prev) = slosh.last {
            omega = (cur - prev) / dt;
        }
        slosh.last = Some(cur);
    }

    // Damped spring toward rest (disp = 0 → level surface, liquid at world-down).
    // Yaw spin shoves the surface laterally in X, pitch in Z; the spring settles
    // it back with an overshoot wobble.
    let impulse = Vec3::new(omega.x, 0.0, omega.y) * SLOSH_GAIN;
    let disp = slosh.disp;
    let vel = slosh.vel;
    let accel = -SLOSH_K * disp - SLOSH_DAMP * vel + impulse;
    let new_vel = vel + accel * dt;
    let mut new_disp = disp + new_vel * dt;
    if new_disp.length() > SLOSH_MAX {
        new_disp = new_disp.normalize() * SLOSH_MAX;
    }
    slosh.vel = new_vel;
    slosh.disp = new_disp;

    // Push the global slosh into every live pipe material instance.
    let s = new_disp.extend(0.0);
    for (_, material) in materials.iter_mut() {
        material.slosh = s;
    }
}
