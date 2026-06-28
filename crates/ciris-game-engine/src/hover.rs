//! Cursor "attention" effect (DESIGN_BRIEF §4.8, interaction polish).
//!
//! Whichever lattice cell the pointer is over — stone or empty — becomes the
//! focus: a soft cursor-follow light makes that spot glow, and the shared
//! [`PlasmaMaterial`] cage rushes/brightens inward toward it (the plasma
//! "breathes in" to the cursor). Everything is smoothed so the focus glides
//! between cells instead of snapping, and fades out when the pointer leaves.
//!
//! Backend-independent (pure ECS + a uniform write), so it runs identically on
//! native, WebGPU, and WebGL2.

use bevy::camera::Camera3d;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::orb::{OrbHandles, OrbMaterial};
use crate::render::cell_world_pos;
use crate::BoardResource;

/// Perpendicular ray-distance (world units) within which a cell counts as
/// "under" the cursor. Cells sit 1 unit apart, so a hair over a half-cell
/// tiles the lattice without gaps or double-claims.
const PICK_RADIUS: f32 = 0.55;
/// Smoothing time constant (s) for the focus position + strength. Small =
/// snappy follow; large = lazy glide. 0.10 s reads as "responsive but smooth".
const TAU: f32 = 0.10;
/// Peak cursor-glow light intensity (lumens), scaled by focus strength.
const GLOW_LUX: f32 = 120_000.0;
/// Radius (world units) the cursor glow light reaches.
const GLOW_RANGE: f32 = 3.2;

/// Smoothed cursor focus, shared by the plasma uniform and the glow light.
#[derive(Resource, Default)]
pub(crate) struct HoverState {
    /// Smoothed world-space focus point.
    pos: Vec3,
    /// Smoothed strength in `[0, 1]` (1 = pointer over a cell, 0 = no focus).
    strength: f32,
}

/// Marker for the single cursor-follow glow light.
#[derive(Component)]
struct HoverLight;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<HoverState>()
        .add_systems(Startup, spawn_hover_light)
        .add_systems(Update, update_hover);
}

/// One dim point light that rides the cursor focus; intensity is driven to 0
/// at rest so it only glows what the pointer is actually over.
fn spawn_hover_light(mut commands: Commands) {
    commands.spawn((
        HoverLight,
        PointLight {
            intensity: 0.0,
            range: GLOW_RANGE,
            radius: 0.0,
            // Warm-cool neutral so it flatters every steward pigment equally.
            color: Color::srgb(0.95, 0.97, 1.0),
            ..default()
        },
        Transform::default(),
    ));
}

/// Cast a ray from the cursor, find the frontmost cell under it, ease the focus
/// toward it, and push the result into the plasma uniform + the glow light.
#[allow(clippy::too_many_arguments)]
fn update_hover(
    time: Res<Time>,
    board: Res<BoardResource>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    orb_handles: Option<Res<OrbHandles>>,
    mut orbs: ResMut<Assets<OrbMaterial>>,
    mut state: ResMut<HoverState>,
    mut light: Query<(&mut PointLight, &mut Transform), With<HoverLight>>,
) {
    let n = board.0.board.n;

    // Resolve the cursor → world ray and the frontmost cell it passes through.
    let target: Option<Vec3> = (|| {
        let window = windows.single().ok()?;
        let cursor = window.cursor_position()?;
        let (camera, cam_tf) = cameras.single().ok()?;
        let ray = camera.viewport_to_world(cam_tf, cursor).ok()?;
        let dir = ray.direction.as_vec3();

        let mut best: Option<(f32, Vec3)> = None; // (t along ray, cell center)
        for idx in 0..board.0.board.len() {
            let center = cell_world_pos(board.0.board.coord(idx), n);
            let t = (center - ray.origin).dot(dir);
            if t <= 0.0 {
                continue;
            }
            let perp = (ray.origin + dir * t).distance(center);
            if perp < PICK_RADIUS && best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, center));
            }
        }
        best.map(|(_, c)| c)
    })();

    // Exponential smoothing toward the target (or toward "off" when none).
    let k = (1.0 - (-time.delta_secs() / TAU).exp()).clamp(0.0, 1.0);
    let goal_strength = if target.is_some() { 1.0 } else { 0.0 };
    if let Some(p) = target {
        // Snap-glide the position only while we have a live target, so the
        // focus doesn't drift through the lattice as it fades out.
        state.pos = state.pos.lerp(p, k);
    }
    state.strength += (goal_strength - state.strength) * k;

    // Drive every orb material's selection uniform (xyz = focus, w = strength)
    // so the sphere nearest the cursor swirls with light.
    if let Some(handles) = orb_handles {
        let hover = state.pos.extend(state.strength);
        for h in &handles.0 {
            if let Some(mut mat) = orbs.get_mut(h) {
                mat.hover = hover;
            }
        }
    }

    // Drive the cursor glow light.
    if let Ok((mut point, mut tf)) = light.single_mut() {
        point.intensity = GLOW_LUX * state.strength;
        tf.translation = state.pos;
    }
}
