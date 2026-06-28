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
pub fn ghost() -> StandardMaterial {
    StandardMaterial {
        base_color: palette::SLATE_SRGB.with_alpha(0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

/// Glass pipe between two face-adjacent same-steward live cells (DESIGN_BRIEF
/// §3.4). The same Borosilicate lens as the [`glass`] shell, but with the
/// `attenuation_color` biased 25 % toward the steward pigment so a mesh's pipes
/// pick up its colour as light passes through them.
pub fn pipe(steward: Steward) -> StandardMaterial {
    let slot = steward.slot() as usize;
    StandardMaterial {
        base_color: palette::BOROSILICATE_SRGB,
        specular_transmission: 0.9,
        ior: 1.50,
        thickness: 0.10,
        perceptual_roughness: 0.06,
        metallic: 0.0,
        reflectance: 0.45,
        attenuation_color: mix_linear(
            palette::BOROSILICATE_LINEAR,
            palette::STEWARD_LINEAR[slot],
            0.25,
        ),
        attenuation_distance: 1.4,
        ..default()
    }
}

/// Base agent-mote emissive intensity before the §4.9 breath modulation.
pub const MOTE_EMISSIVE: f32 = 2.4;

/// Emissive colour of an agent mote for steward `slot`, scaled by `factor`
/// (DESIGN_BRIEF §3.9 / §4.3). The pigment is lifted ~15 % toward white (the
/// §3.9 OKLCH L*+8 % shift, approximated in linear RGB) so the motes read a
/// touch brighter than the core they orbit; `factor` carries the §4.9 atari
/// breath (`1.0` at rest). The effects layer drives each mote's own material
/// with this so the breath can pulse one mesh without touching its neighbours.
pub fn mote_emissive(slot: usize, factor: f32) -> LinearRgba {
    let glow = mix_linear(palette::STEWARD_LINEAR[slot], Color::WHITE, 0.15);
    glow.to_linear() * (MOTE_EMISSIVE * factor)
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

/// Linear-space blend of two colours, `t` of the way from `a` to `b`.
fn mix_linear(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    Color::LinearRgba(LinearRgba::rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    ))
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
