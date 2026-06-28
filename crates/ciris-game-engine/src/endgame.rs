//! Endgame ceremonies (DESIGN_BRIEF §4.7): MOURN, CELEBRATE, and WILD. When the
//! board reaches `is_over()` the screensaver holds on the final frame for a few
//! seconds (`screensaver.rs`); this module paints that hold.
//!
//! The ending is classified from the final board (§4.7 `proximity_to_dead` /
//! `any_dead_globally`) and then drives global ambiance only — bloom, ambient
//! illuminance, and a slow camera sweep — plus, for WILD, the in-engine line
//! "The federation held." (Bevy UI text; no DOM). It restores the baseline when
//! the screensaver reseeds. It never touches game state.

use bevy::light::GlobalAmbientLight;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::palette;
use crate::screensaver::ScreensaverState;
use crate::BoardResource;
use ciris_game_engine_core::CellState;

/// Bloom intensity at rest (mirrors the camera spawn in `render.rs`).
const BLOOM_BASE: f32 = 0.18;
/// Ambient brightness at rest (mirrors `lighting.rs`).
const AMBIENT_BASE: f32 = 350.0;

/// WILD bloom-pulse peak intensity (§4.7 screen-wide pulse).
const WILD_BLOOM_PEAK: f32 = 0.65;
/// WILD ambient peak — lifts the whole packed lattice into the glow.
const WILD_AMBIENT_PEAK: f32 = 1500.0;
/// WILD hold seconds before the screensaver reseeds (§4.7: 18 s).
const WILD_HOLD_SECS: f32 = 18.0;
/// WILD camera sweep — 540° over 12 s (§4.7).
const WILD_SWEEP_RATE: f32 = std::f32::consts::PI * 3.0 / 12.0;

/// CELEBRATE bloom multiplier (§4.7: ×1.3) and brisk sweep.
const CELEBRATE_BLOOM: f32 = BLOOM_BASE * 1.3;
const CELEBRATE_SWEEP_RATE: f32 = 0.45;
/// MOURN bloom multiplier (§4.7: ×0.7) and a slow 0.3 Hz inhale.
const MOURN_BLOOM: f32 = BLOOM_BASE * 0.7;
const MOURN_BREATH_HZ: f32 = 0.3;
const MOURN_SWEEP_RATE: f32 = 0.04;

/// Which ending is playing (DESIGN_BRIEF §4.7).
#[derive(Clone, Copy, PartialEq)]
enum EndKind {
    /// No perma-dead anywhere — the M-1 cooperative ending.
    Wild,
    /// Perma-dead exists and a survivor sits beside it.
    Mourn,
    /// Perma-dead exists but no survivor touches it.
    Celebrate,
}

/// Live ending state. `None` while a game is in progress.
#[derive(Resource, Default)]
pub(crate) struct Ending {
    kind: Option<EndKind>,
    /// `Time::elapsed_secs` when the ending began.
    start: f32,
    /// The WILD "The federation held." UI root, despawned on reset.
    text: Option<Entity>,
}

/// Classify the finished board (§4.7). `proximity_to_dead` is evaluated globally:
/// any surviving live cell face-adjacent to any perma-dead → MOURN.
fn classify(gs: &ciris_game_engine_core::GameState) -> EndKind {
    let b = &gs.board;
    let any_dead = (0..b.len()).any(|i| b.get(i) == CellState::PermaDead);
    if !any_dead {
        return EndKind::Wild;
    }
    let near_dead = (0..b.len()).any(|i| {
        matches!(b.get(i), CellState::Live(_))
            && b.neighbors(i)
                .iter()
                .any(|&nb| b.get(nb) == CellState::PermaDead)
    });
    if near_dead {
        EndKind::Mourn
    } else {
        EndKind::Celebrate
    }
}

/// Drive the endgame ceremony each frame while the board is over, and restore the
/// baseline once the screensaver reseeds (DESIGN_BRIEF §4.7).
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_endgame(
    time: Res<Time>,
    board: Res<BoardResource>,
    mut ending: ResMut<Ending>,
    mut screensaver: ResMut<ScreensaverState>,
    mut cam: Query<(&mut Bloom, &mut PanOrbitCamera)>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut commands: Commands,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let over = board.0.is_over();

    // Game resumed (reseed): tear the ceremony down and restore the baseline.
    if !over {
        if ending.kind.take().is_some() {
            if let Some(e) = ending.text.take() {
                commands.entity(e).despawn();
            }
            if let Ok((mut bloom, _)) = cam.single_mut() {
                bloom.intensity = BLOOM_BASE;
            }
            ambient.brightness = AMBIENT_BASE;
        }
        return;
    }

    // First frame of the hold: classify and arm the ceremony.
    if ending.kind.is_none() {
        let kind = classify(&board.0);
        ending.start = t;
        if kind == EndKind::Wild {
            ending.text = Some(spawn_wild_text(&mut commands));
            screensaver.set_hold(WILD_HOLD_SECS);
        }
        ending.kind = Some(kind);
    }

    let Some(kind) = ending.kind else {
        return;
    };
    let e = t - ending.start;
    let Ok((mut bloom, mut orbit)) = cam.single_mut() else {
        return;
    };

    match kind {
        EndKind::Wild => {
            let pulse = wild_pulse(e);
            bloom.intensity = BLOOM_BASE + (WILD_BLOOM_PEAK - BLOOM_BASE) * pulse;
            ambient.brightness = AMBIENT_BASE + (WILD_AMBIENT_PEAK - AMBIENT_BASE) * pulse;
            orbit.target_yaw += WILD_SWEEP_RATE * dt;
        }
        EndKind::Celebrate => {
            bloom.intensity = CELEBRATE_BLOOM;
            orbit.target_yaw += CELEBRATE_SWEEP_RATE * dt;
        }
        EndKind::Mourn => {
            bloom.intensity = MOURN_BLOOM;
            // 0.3 Hz inhale on the ambient term — a slow, grieving breath.
            let breath = 0.85 + 0.15 * (std::f32::consts::TAU * MOURN_BREATH_HZ * e).sin();
            ambient.brightness = AMBIENT_BASE * breath;
            orbit.target_yaw += MOURN_SWEEP_RATE * dt;
        }
    }
}

/// The §4.7 WILD bloom envelope: a 5 s pulse — ramp in (1 s), hold (to 4 s), ease
/// out (to 6 s), then rest so the held line lingers on the calm lattice.
fn wild_pulse(e: f32) -> f32 {
    if e < 1.0 {
        smoothstep01(e)
    } else if e < 4.0 {
        1.0
    } else if e < 6.0 {
        1.0 - smoothstep01((e - 4.0) / 2.0)
    } else {
        0.0
    }
}

fn smoothstep01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Spawn the centred "The federation held." line (§4.7). In-engine Bevy UI only.
fn spawn_wild_text(commands: &mut Commands) -> Entity {
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_children(|root| {
            root.spawn((
                Text::new("The federation held."),
                TextFont {
                    font_size: FontSize::Px(44.0),
                    ..default()
                },
                TextColor(palette::INK_SRGB),
            ));
        })
        .id()
}
