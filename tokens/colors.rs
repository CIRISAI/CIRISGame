//! CIRISGame palette — generated from tokens.json. Do not hand-edit.
//! `*_SRGB` for UI & StandardMaterial.base_color; `*_LINEAR` for emissive,
//! attenuation_color, and light tints (Bevy lights expect linear).

use bevy::prelude::Color;

/// Bone — Light-mode page background
pub const BONE_SRGB: Color = Color::srgb(0.9804, 0.9765, 0.9608);
pub const BONE_LINEAR: Color = Color::linear_rgb(0.95597, 0.94731, 0.9131);

/// Linen — Side-panel / HUD chrome fill
pub const LINEN_SRGB: Color = Color::srgb(0.9098, 0.9020, 0.8627);
pub const LINEN_LINEAR: Color = Color::linear_rgb(0.80695, 0.7913, 0.71569);

/// Borosilicate — Multiplied over shell transmission
pub const BOROSILICATE_SRGB: Color = Color::srgb(0.9176, 0.9490, 0.9333);
pub const BOROSILICATE_LINEAR: Color = Color::linear_rgb(0.82279, 0.88792, 0.85499);

/// Ink — Body type, sphere rim, pipe joints
pub const INK_SRGB: Color = Color::srgb(0.0784, 0.0784, 0.0745);
pub const INK_LINEAR: Color = Color::linear_rgb(0.007, 0.007, 0.00651);

/// Slate — Hairlines, secondary type, wireframe
pub const SLATE_SRGB: Color = Color::srgb(0.3725, 0.3608, 0.3333);
pub const SLATE_LINEAR: Color = Color::linear_rgb(0.11444, 0.10702, 0.09084);

/// Stone — Dead-pipe stone, hairline accents
pub const STONE_SRGB: Color = Color::srgb(0.6902, 0.6824, 0.6471);
pub const STONE_LINEAR: Color = Color::linear_rgb(0.43415, 0.42327, 0.37626);

/// Clay — Turn indicator, hover ring, victory flash
pub const CLAY_SRGB: Color = Color::srgb(0.8510, 0.4667, 0.3412);
pub const CLAY_LINEAR: Color = Color::linear_rgb(0.69387, 0.18447, 0.09531);

/// Lapis — Focus state, thinking pulse
pub const LAPIS_ACCENT_SRGB: Color = Color::srgb(0.4157, 0.6078, 0.8000);
pub const LAPIS_ACCENT_LINEAR: Color = Color::linear_rgb(0.14413, 0.32778, 0.60383);

/// Ochre — Horizon dome lower band
pub const OCHRE_SRGB: Color = Color::srgb(0.8314, 0.6353, 0.4980);
pub const OCHRE_LINEAR: Color = Color::linear_rgb(0.65837, 0.36131, 0.21223);

// ── steward pigment cores ──────────────────────────────────────────

/// Burnt Sienna (seed: spots)
pub const STEWARD_SIENNA_SRGB: Color = Color::srgb(0.8510, 0.4667, 0.3412);
pub const STEWARD_SIENNA_LINEAR: Color = Color::linear_rgb(0.69387, 0.18447, 0.09531);

/// Lapis Lazuli (seed: stripes)
pub const STEWARD_LAPIS_SRGB: Color = Color::srgb(0.4157, 0.6078, 0.8000);
pub const STEWARD_LAPIS_LINEAR: Color = Color::linear_rgb(0.14413, 0.32778, 0.60383);

/// Verdigris (seed: labyrinth)
pub const STEWARD_VERDIGRIS_SRGB: Color = Color::srgb(0.4706, 0.5490, 0.3647);
pub const STEWARD_VERDIGRIS_LINEAR: Color = Color::linear_rgb(0.18782, 0.26225, 0.10946);

/// Kaolin (seed: spirals) — render with mandatory 2px Ink ring
pub const STEWARD_KAOLIN_SRGB: Color = Color::srgb(0.9098, 0.9020, 0.8627);
pub const STEWARD_KAOLIN_LINEAR: Color = Color::linear_rgb(0.80695, 0.7913, 0.71569);

pub const STEWARD_SRGB: [Color; 4] = [STEWARD_SIENNA_SRGB, STEWARD_LAPIS_SRGB, STEWARD_VERDIGRIS_SRGB, STEWARD_KAOLIN_SRGB];
pub const STEWARD_LINEAR: [Color; 4] = [STEWARD_SIENNA_LINEAR, STEWARD_LAPIS_LINEAR, STEWARD_VERDIGRIS_LINEAR, STEWARD_KAOLIN_LINEAR];
