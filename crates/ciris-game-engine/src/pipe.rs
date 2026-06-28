//! Swirling-gas material (DESIGN_BRIEF §3.3/§3.4): a solid-colour steward gas
//! that slowly swirls. The same material fills two things — the inside of a live
//! sphere's clear glass shell ([`core_material`]) and the fat pipe between two
//! connected same-colour spheres ([`material`]). No patterns (distracting), no
//! physics; just a gently churning solid colour.
//!
//! webgl2-safe [`AsBindGroup`] fragment material (shader `assets/shaders/pipe.wgsl`,
//! bind group 3): fragment-only, constant loop bounds, vec4-aligned uniforms.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::palette;
use ciris_game_engine_core::Steward;

// ── pipe gas (the channel between two connected spheres) ────────────────────
/// Pipe gas opacity — translucent so the channel still reads as glass with a
/// coloured vapour inside, not a solid rod.
const PIPE_ALPHA: f32 = 0.62;
/// Swirl speed (rad/s of domain rotation).
const PIPE_SWIRL_SPEED: f32 = 0.5;
/// Swirl scale (larger lobes at lower values → less "pattern", more "gas").
const PIPE_SWIRL_SCALE: f32 = 2.2;
/// Glow gain — modestly lit so the pipe reads as vapour, not a light source.
const PIPE_GLOW: f32 = 1.05;
/// Solidity — high so the pipe is mostly one solid colour with a gentle swirl.
const PIPE_SOLIDITY: f32 = 0.55;

/// The swirling-gas material (DESIGN_BRIEF §3.3/§3.4).
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PipeMaterial {
    /// rgb = steward pigment (linear), a = gas opacity.
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = swirl speed, y = swirl scale, z = glow gain, w = solidity.
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

fn pigment(steward: Steward, alpha: f32) -> LinearRgba {
    let rgba = palette::STEWARD_LINEAR[steward.slot() as usize].to_linear();
    LinearRgba::new(rgba.red, rgba.green, rgba.blue, alpha)
}

/// Gas material for a **pipe** between two connected spheres, in the steward's
/// pigment.
pub(crate) fn material(steward: Steward) -> PipeMaterial {
    PipeMaterial {
        color: pigment(steward, PIPE_ALPHA),
        params: Vec4::new(PIPE_SWIRL_SPEED, PIPE_SWIRL_SCALE, PIPE_GLOW, PIPE_SOLIDITY),
    }
}

/// Register the pipe material plugin. Added from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PipeMaterial>::default());
}
