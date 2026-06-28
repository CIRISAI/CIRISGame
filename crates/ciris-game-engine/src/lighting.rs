//! The DESIGN_BRIEF §2.2 three-point lighting rig plus the global ambient that
//! folds in the §2.2 "sky" term until the §3.8 IBL dome lands. Light *positions*
//! scale by `N/5`; the tints and illuminance multipliers are the N = 5 baseline.

use bevy::light::GlobalAmbientLight;
use bevy::prelude::*;

use crate::palette;

/// Base illuminance (lux) the §2.2 intensity multipliers scale.
const BASE_LUX: f32 = 10_000.0;

/// Spawn the key / fill / rim directional lights and the ambient sky term.
/// `scale = N/5` (DESIGN_BRIEF §2.2). Only the key casts shadows — fill and rim
/// stay shadowless so the webgl2 deploy target keeps a single shadow map.
pub fn spawn_rig(commands: &mut Commands, scale: f32) {
    // Key — warm clay, upper-right-front; the only shadow caster.
    spawn_dir(
        commands,
        scale,
        Vec3::new(6.67, 9.17, 5.00),
        Color::srgb_u8(0xFF, 0xE5, 0xCC),
        BASE_LUX * 1.1,
        true,
    );
    // Fill — cool linen, lower-left, half strength.
    spawn_dir(
        commands,
        scale,
        Vec3::new(-5.83, 2.50, 4.17),
        Color::srgb_u8(0xDC, 0xE5, 0xEF),
        BASE_LUX * 0.55,
        false,
    );
    // Rim — accent rake from behind, narrow read.
    spawn_dir(
        commands,
        scale,
        Vec3::new(0.83, 3.67, -7.50),
        Color::srgb_u8(0xFF, 0xD6, 0xA8),
        BASE_LUX * 1.2,
        false,
    );

    // Sky — hemispheric overhead (§2.2). Folded into ambient until the §3.8
    // procedural-HDR dome feeds real image-based lighting. Borosilicate tint
    // keeps shadows cool against the warm key.
    commands.insert_resource(GlobalAmbientLight {
        color: palette::BOROSILICATE_SRGB,
        // Low ambient against the dark dome so the glow reads (was 350, which
        // flooded the scene to a pale wash).
        brightness: 70.0,
        ..default()
    });
}

/// A directional light aimed at the board centre from `pos`, scaled by `N/5`.
fn spawn_dir(
    commands: &mut Commands,
    scale: f32,
    pos: Vec3,
    color: Color,
    lux: f32,
    shadows: bool,
) {
    commands.spawn((
        DirectionalLight {
            color,
            illuminance: lux,
            shadow_maps_enabled: shadows,
            ..default()
        },
        Transform::from_translation(pos * scale).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
