//! Steward orb material (DESIGN_BRIEF §3.2/§3.3): one transparent sphere that
//! reads as a thick clear glass shell with two swirling gasses inside it (a faint
//! clear gas + a bright neon coloured gas). A single surface, so the gas is never
//! depth-occluded by a separate glass shell. Shader: `assets/shaders/orb.wgsl`.
//!
//! Two flavours:
//! * [`material`] — a live steward sphere: neon coloured gas.
//! * [`empty_material`] — a tiny clear, slightly-grey glass sphere marking a
//!   lattice position where a sphere *could* be placed.
//!
//! Both honour a shared `hover` uniform: the sphere nearest the cursor "swirls
//! with light" to show it is selected (`hover.rs` drives it).
//!
//! webgl2-safe [`AsBindGroup`] fragment material (bind group 3): fragment-only,
//! constant loop bounds, vec4-aligned uniforms, alpha-blended.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::palette;
use ciris_game_engine_core::Steward;

// ── live steward orb (neon) ─────────────────────────────────────────────────
/// Opaque so the surrounding clear glass shell refracts it into a marble (a
/// transparent core isn't captured by `specular_transmission`).
const BASE_ALPHA: f32 = 1.0;
/// Swirl speed (rad/s of domain rotation) — slow, a lazy churn.
const SWIRL_SPEED: f32 = 0.15;
/// Swirl scale (lobes per unit; lower = larger, gassier lobes).
const SWIRL_SCALE: f32 = 2.8;
/// Core brightness. Modest on purpose: the steward pigments are muted earth
/// tones, so pushing glow high just blooms them to white (pastel) and erases the
/// glass edge. We stay just into HDR and get "neon" from saturation instead.
const NEON_GLOW: f32 = 2.2;
/// Fresnel "glass" rim gain — bright wide edge catch so it reads as thick glass.
const RIM_GAIN: f32 = 3.0;

// ── empty-position marker (tiny clear grey glass) ───────────────────────────
/// Slight grey tint (linear) for the empty-position spheres.
const EMPTY_TINT: LinearRgba = LinearRgba::new(0.55, 0.58, 0.62, 0.32);
const EMPTY_GLOW: f32 = 1.4;
const EMPTY_RIM: f32 = 2.0;

/// The steward orb material.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct OrbMaterial {
    /// rgb = pigment (linear), a = base centre opacity.
    #[uniform(0)]
    pub color: LinearRgba,
    /// x = swirl speed, y = swirl scale, z = neon glow gain, w = rim gain.
    #[uniform(1)]
    pub params: Vec4,
    /// Cursor selection: `xyz` = world focus point, `w` = strength `[0,1]`. The
    /// sphere nearest the focus swirls brighter with light. `w = 0` is resting.
    #[uniform(2)]
    pub hover: Vec4,
}

impl Material for OrbMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/orb.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // Opaque live cores (so glass can refract them into marbles); translucent
        // empty-position markers.
        if self.color.alpha >= 0.999 {
            AlphaMode::Opaque
        } else {
            AlphaMode::Blend
        }
    }
}

/// A live steward sphere — neon coloured swirling gas in thick clear glass.
pub(crate) fn material(steward: Steward) -> OrbMaterial {
    let rgba = palette::STEWARD_LINEAR[steward.slot() as usize].to_linear();
    OrbMaterial {
        color: LinearRgba::new(rgba.red, rgba.green, rgba.blue, BASE_ALPHA),
        params: Vec4::new(SWIRL_SPEED, SWIRL_SCALE, NEON_GLOW, RIM_GAIN),
        hover: Vec4::ZERO,
    }
}

/// A tiny clear, slightly-grey glass sphere marking an empty lattice position.
pub(crate) fn empty_material() -> OrbMaterial {
    OrbMaterial {
        color: EMPTY_TINT,
        params: Vec4::new(SWIRL_SPEED, SWIRL_SCALE, EMPTY_GLOW, EMPTY_RIM),
        hover: Vec4::ZERO,
    }
}

/// Every orb material handle (empty marker + the four steward colours), so
/// `hover.rs` can drive the selection uniform on each one per frame.
#[derive(Resource)]
pub(crate) struct OrbHandles(pub Vec<Handle<OrbMaterial>>);

/// Register the orb material plugin. Added from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<OrbMaterial>::default());
}
