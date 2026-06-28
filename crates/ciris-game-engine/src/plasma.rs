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
const TINT: LinearRgba = LinearRgba::new(0.34, 0.58, 0.84, 0.55);
/// x = flow speed, y = spatial freq, z = floor brightness, w = glow gain.
/// Glow gain is pushed well past 1.0 so the wave peaks land deep in HDR and
/// Bloom bleeds the thin strands into a soft glowing cage (so it reads as
/// *plasma*, not flat 1-px wireframe). Floor keeps a calm constant haze.
const PARAMS: Vec4 = Vec4::new(0.55, 1.25, 0.36, 3.4);

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PlasmaMaterial {
    #[uniform(0)]
    pub tint: LinearRgba,
    #[uniform(1)]
    pub params: Vec4,
    /// Cursor "attention": `xyz` = world-space focus point, `w` = strength
    /// `[0,1]`. The shader makes the cage brighten and rush inward toward this
    /// point (`hover.rs` drives it every frame); `w = 0` is the resting cage.
    #[uniform(2)]
    pub hover: Vec4,
}

impl Default for PlasmaMaterial {
    fn default() -> Self {
        Self {
            tint: TINT,
            params: PARAMS,
            hover: Vec4::ZERO,
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

/// The single shared plasma material handle (every empty cell clones it), so
/// `hover.rs` can drive the cursor-attention uniform on one material per frame.
#[derive(Resource)]
pub(crate) struct PlasmaHandle(pub Handle<PlasmaMaterial>);

/// Register the plasma material plugin. Added from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<PlasmaMaterial>::default());
}
