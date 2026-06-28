//! Flowing-plasma ghost wireframe (DESIGN_BRIEF §3.5, reimagined).
//!
//! The empty-cell lattice is drawn as gently flowing, translucent, ethereal
//! plasma lines (the "prayer ball" openwork) rather than flat Slate hairlines.
//! A webgl2-safe [`AsBindGroup`] fragment material on the `LineList` ghost mesh:
//! the colour flows along world space over time so the cage shimmers. The grid
//! shows only where stones/pipes aren't — `sync_board` swaps the frame to a glass
//! shell the instant a cell goes live.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// Ethereal plasma tint (linear), `a` = base opacity. Cool Lapis-cyan so the
/// cage reads as a calm electric haze against the warm void + warm pigment cores.
const TINT: LinearRgba = LinearRgba::new(0.34, 0.58, 0.84, 0.4);
/// x = flow speed, y = spatial freq, z = floor brightness, w = glow gain.
/// Gentle + ethereal: slow drift, soft large waves, a soft constant haze with
/// quiet peaks (so the cage reads as a calm field, not a busy neon web).
const PARAMS: Vec4 = Vec4::new(0.5, 1.2, 0.28, 1.15);

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PlasmaMaterial {
    #[uniform(0)]
    pub tint: LinearRgba,
    #[uniform(1)]
    pub params: Vec4,
}

impl Default for PlasmaMaterial {
    fn default() -> Self {
        Self {
            tint: TINT,
            params: PARAMS,
        }
    }
}

impl Material for PlasmaMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/plasma.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Register the plasma material plugin. Added from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PlasmaMaterial>::default());
}
