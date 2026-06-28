//! Gas-pigment pipe material (DESIGN_BRIEF §3.4): the steward pigment drifts as a
//! soft luminous **gas** through the glass channel between connected same-colour
//! cells — translucent, slowly churning, no physics (no gravity fill, no slosh).
//!
//! Like [`crate::mist`] this is a webgl2-safe [`AsBindGroup`] fragment material
//! (shader `assets/shaders/pipe.wgsl`, bind group 3): fragment-only, constant
//! loop bounds, vec4-aligned uniforms.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::palette;
use ciris_game_engine_core::Steward;

/// Gas opacity (peak alpha of the pigment column) — translucent so the channel
/// still reads as glass with a coloured vapour inside, not a solid rod.
const GAS_ALPHA: f32 = 0.5;
/// Drift speed (units/s) of the gas noise.
const FLOW_SPEED: f32 = 0.35;
/// Gas noise frequency (churning lobes per unit).
const FLOW_FREQ: f32 = 6.0;
/// Gas density gain — higher = thicker, more opaque vapour.
const DENSITY: f32 = 1.15;

/// The per-pipe gas material (DESIGN_BRIEF §3.4).
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PipeMaterial {
    /// rgb = steward pigment (linear), a = gas opacity.
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = flow speed, y = noise freq, z = density, w = unused.
    #[uniform(1)]
    pub params: Vec4,
}

impl Material for PipeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/pipe.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Build a pipe's gas material in the steward's pigment.
pub(crate) fn material(steward: Steward) -> PipeMaterial {
    let slot = steward.slot() as usize;
    let rgba = palette::STEWARD_LINEAR[slot].to_linear();
    PipeMaterial {
        color: LinearRgba::new(rgba.red, rgba.green, rgba.blue, GAS_ALPHA),
        params: Vec4::new(FLOW_SPEED, FLOW_FREQ, DENSITY, 0.0),
    }
}

/// Register the pipe material plugin. Added from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PipeMaterial>::default());
}
