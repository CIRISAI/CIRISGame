//! `StandardMaterial` builders for the lattice presentation (DESIGN_BRIEF §3.2
//! glass shell, §3.3 emissive core, §3.6 dead-group shells) plus the static
//! Gray-Scott pigment seed (§4.2) that gives each steward its pattern family.
//!
//! Colour discipline (see `palette.rs`): `base_color` / texture albedo take the
//! `*_SRGB` tokens; `emissive` and `attenuation_color` take the linear ones.

use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::palette;
use ciris_game_engine_core::Steward;

/// Inner-core emissive intensity (DESIGN_BRIEF §3.3, range `[0.4, 1.8]`). Tuned
/// near the top of the range so the cores read as lit pigment through the §2.3
/// bloom rather than as flat matte spheres.
pub const CORE_EMISSIVE: f32 = 1.5;

/// Kaolin's mandatory 2 px Ink ring (§2.1 / §3.3) is approximated with an
/// inverted-hull outline sphere at this fraction over the core radius.
pub const KAOLIN_RING_SCALE: f32 = 1.12;

/// Glass shell (DESIGN_BRIEF §3.2). Real `specular_transmission` refraction with
/// a faint Borosilicate tint. On the webgl2 deploy target the screen-space
/// refraction is softer (§1) but the material still reads as a clean lens.
///
/// TODO §3.2: add the Fresnel rim term via
/// `ExtendedMaterial<StandardMaterial, RimMaterial>` for the visionOS edge-catch.
pub fn glass() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::BOROSILICATE_SRGB,
        specular_transmission: 0.9,
        ior: 1.50,
        thickness: 0.18,
        perceptual_roughness: 0.06,
        metallic: 0.0,
        reflectance: 0.45,
        attenuation_color: palette::BOROSILICATE_LINEAR,
        attenuation_distance: 2.2,
        ..default()
    }
}

/// Emissive steward-core material (DESIGN_BRIEF §3.3). `pattern` is the per-
/// steward Gray-Scott seed baked to a pigment mask ([`bake_pigment_mask`]); it
/// modulates both albedo and glow so the core reads as pigment that is brighter
/// along the steward's pattern family (spots / stripes / labyrinth / spirals).
pub fn core(steward: Steward, pattern: Option<Handle<Image>>) -> StandardMaterial {
    let slot = steward.slot() as usize;
    StandardMaterial {
        base_color: palette::STEWARD_SRGB[slot],
        base_color_texture: pattern.clone(),
        emissive: palette::STEWARD_LINEAR[slot].to_linear() * CORE_EMISSIVE,
        emissive_texture: pattern,
        perceptual_roughness: 0.45,
        // TODO §4.2: replace this static seed with the live ping-pong R-D sim.
        ..default()
    }
}

/// Inverted-hull outline for Kaolin's core (DESIGN_BRIEF §3.3). `cull_mode =
/// Front` renders only the back faces of a slightly larger Ink sphere, leaving a
/// thin dark silhouette around the near-Bone core. Stays off the bloom layer so
/// the rim never glows.
pub fn kaolin_ring() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::INK_SRGB,
        unlit: true,
        cull_mode: Some(Face::Front),
        ..default()
    }
}

/// Temp-dead shell: desaturated and dark, transmission killed (DESIGN_BRIEF §3.6).
///
/// TODO §3.6: raymarched black volumetric mist flowing inside the shell.
pub fn tempdead() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB,
        perceptual_roughness: 0.9,
        reflectance: 0.2,
        ..default()
    }
}

/// Perma-dead shell: Verdigris-tinted neutral substrate (DESIGN_BRIEF §3.6).
///
/// TODO §3.6: slow green mist at rest.
pub fn permadead() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::STEWARD_VERDIGRIS_SRGB,
        specular_transmission: 0.3,
        ior: 1.50,
        thickness: 0.18,
        perceptual_roughness: 0.25,
        attenuation_color: palette::STEWARD_VERDIGRIS_LINEAR,
        attenuation_distance: 1.4,
        reflectance: 0.35,
        ..default()
    }
}

/// Faint ghost marker for empty cells (DESIGN_BRIEF §3.5 placeholder).
///
/// TODO §3.5: `bevy_polyline` rhombic-dodecahedron wireframe with distance fade.
pub fn ghost() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB.with_alpha(0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

/// Bake a raw Gray-Scott seed PNG into a grayscale pigment mask, in place.
///
/// The seed art encodes the reaction-diffusion field as red background with the
/// V-concentration in the green channel; that green channel *is* the pattern
/// family (DESIGN_BRIEF §4.2). We collapse it to a `[0.4, 1.0]` luminance so the
/// core's steward `emissive`/`base_color` stays on-pigment everywhere and simply
/// brightens along the pattern — the static Tier-A approximation of the live sim.
pub fn bake_pigment_mask(image: &mut Image) {
    let (w, h) = (image.width(), image.height());
    for y in 0..h {
        for x in 0..w {
            if let Ok(c) = image.get_color_at(x, y) {
                // Green channel carries the pattern; remap to a pigment mask.
                let v = c.to_srgba().green;
                let l = 0.4 + 0.6 * v;
                let _ = image.set_color_at(x, y, Color::srgb(l, l, l));
            }
        }
    }
}
