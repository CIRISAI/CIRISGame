//! `StandardMaterial` builders for the lattice presentation (DESIGN_BRIEF §3.2
//! glass shell, §3.3 emissive core, §3.6 dead-group shells) plus the static
//! Gray-Scott pigment seed (§4.2) that gives each steward its pattern family.
//!
//! Colour discipline (see `palette.rs`): `base_color` / texture albedo take the
//! `*_SRGB` tokens; `emissive` and `attenuation_color` take the linear ones.

use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::palette;

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
        // Clear lens: near-total transmission + long attenuation so it tints
        // almost nothing, and a thick wall + clean roughness so the refraction of
        // the opaque core reads as a marble (you see the glass edge bend it).
        specular_transmission: 0.97,
        ior: 1.52,
        thickness: 0.6,
        perceptual_roughness: 0.03,
        metallic: 0.0,
        reflectance: 0.5,
        attenuation_color: palette::BOROSILICATE_LINEAR,
        attenuation_distance: 12.0,
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

/// Temp-dead shell: a dimmed, desaturated lens — saturation dropped ~60 % and the
/// tint pushed dark (DESIGN_BRIEF §3.6). Kept lightly transmissive so the black
/// volumetric mist ([`crate::mist`]) flowing inside reads through the shell, the
/// same way the live glass refracts its emissive core (§3.2).
pub fn tempdead() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB,
        specular_transmission: 0.6,
        ior: 1.50,
        thickness: 0.18,
        perceptual_roughness: 0.3,
        attenuation_color: palette::SLATE_LINEAR,
        attenuation_distance: 0.9,
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

/// Ghost-cell wireframe material (DESIGN_BRIEF §3.5). Slate at 18 % alpha, unlit
/// so the rhombic-dodecahedron line mesh ([`crate::geometry`]) reads as a calm
/// lattice scaffold rather than a lit surface.
///
/// `bevy_polyline` (which carries the §3.5 per-pixel distance fade for free) has
/// no Bevy-0.19 release — it targets 0.17 — so this falls back to a built-in
/// `LineList` mesh + this material. The distance fade is therefore deferred until
/// either the crate catches up or a small custom WGSL fade material lands.
///
/// Currently unused — the empty cell uses the `plasma` material — but kept for
/// the flat-view / fallback path.
#[allow(dead_code)]
pub fn ghost() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB.with_alpha(0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

/// Verdigris foreshadowing ring for an atari cell (DESIGN_BRIEF §3.9 / §4.9). A
/// faint translucent torus that fades the green mist's colour in before the
/// destructive transition. Unlit + 25 % alpha; shown only for `|M| = 6` cells.
pub fn atari_ring() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::STEWARD_VERDIGRIS_SRGB.with_alpha(0.25),
        emissive: palette::STEWARD_VERDIGRIS_LINEAR.to_linear() * 0.6,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}
