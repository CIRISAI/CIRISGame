//! Layer-traversal fly-through (DESIGN_BRIEF §4.8).
//!
//! [`bevy_panorbit_camera`] gives us orbit + zoom-to-radius, but §4.8 wants the
//! viewer to fly *through* the lattice — "faces flow behind you" — so the interior
//! state can be read from any angle, including from inside the volume. Panorbit's
//! wheel/pinch zoom stops at the focus point; it never travels past it.
//!
//! Two systems, layered on top of the existing `PanOrbitCamera` rig (orbit /
//! rotation is left entirely to panorbit — we touch neither its yaw/pitch nor the
//! `Transform` it owns):
//!
//! * [`fly_through`] — scroll-up / pinch-in dollies the rig FORWARD along the view
//!   direction (and past board centre), scroll-down / pinch-out reverses. It does
//!   this by translating the panorbit `focus` (and `target_focus`) along the
//!   camera's forward axis: panorbit derives the camera position from
//!   `focus + radius·orientation`, so moving the focus translates the whole rig
//!   forward while leaving radius + orientation — and therefore orbit — untouched.
//!   Speed is capped at `FORWARD_SPEED_CAP` world-units/s and exponentially
//!   smoothed with time-constant `SMOOTHING_TAU` (§4.8 knobs
//!   `camera.layerForwardSpeedCap` / `camera.layerSmoothingTau`).
//!
//! * [`update_near_clip`] — the "flow behind you" near-plane. While the camera is
//!   inside the board AABB (±[`AABB_EPS`]), the perspective near plane is pushed
//!   forward so the cells the camera is currently passing through stop occluding
//!   the interior; outside the AABB it rests at [`NEAR_OUTSIDE`] so the ordinary
//!   approach view isn't clipped.
//!
//! Both run only in the render build (the module is `#[cfg(feature = "render")]`
//! in `lib.rs`). The `webgl2` artifact links this unchanged — no post-process or
//! custom-shader work, just stock camera + projection mutation.

use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::BoardResource;

/// World-units/s ceiling on the forward dolly (knob `camera.layerForwardSpeedCap`).
const FORWARD_SPEED_CAP: f32 = 1.5;
/// Exponential-smoothing time-constant, seconds (knob `camera.layerSmoothingTau`).
const SMOOTHING_TAU: f32 = 0.20;
/// World-unit offset added past the clipped slab (knob `camera.nearClipOffset`).
const NEAR_CLIP_OFFSET: f32 = 0.05;
/// Near plane when the camera is outside the lattice volume — matches Bevy's
/// default perspective near so the §4.4 approach zoom isn't fought.
const NEAR_OUTSIDE: f32 = 0.10;
/// Tolerance band on the AABB test, so the near-clip mode doesn't flicker right
/// at the boundary (§4.8 "inside the AABB ± 0.05").
const AABB_EPS: f32 = 0.05;
/// Cell-centre pitch (cores sit on the unit lattice, DESIGN_BRIEF §3.1). Used to
/// size the near-clip slab so roughly the cell the camera occupies is clipped.
const CELL_PITCH: f32 = 1.0;

/// One wheel "line" (a mouse notch) → this much desired forward velocity. A couple
/// of notches builds to the cap; coasting then decays it back to rest.
const WHEEL_LINE_GAIN: f32 = 1.5;
/// Pixel-unit wheel deltas (trackpads, some browsers) are ~16× a line.
const WHEEL_PIXELS_PER_LINE: f32 = 16.0;
/// Trackpad / touch pinch magnification → desired forward velocity.
const PINCH_GAIN: f32 = 12.0;
/// Both-mouse-buttons drag: vertical pixels of motion → desired forward velocity
/// (drag up = forward). Mouse users without a wheel get the §4.8 dolly this way.
const DUAL_MOUSE_GAIN: f32 = 0.06;
/// Time-constant (s) for the desired velocity to coast back to zero once input
/// stops, so each scroll burst glides to a halt rather than running forever.
const COAST_TAU: f32 = 0.18;

/// Half-side of the world starfield cube (side = DEFAULT_BOARD_N * 18.0 for n=5 →
/// half = 45.0). Clamping the focus to this bound keeps the whole rig inside.
const WORLD_CUBE_HALF: f32 = 45.0;
/// Inner bound for focus clamping — a small margin so the camera never clips the
/// cube wall even when the orbit radius is near its maximum.
const FOCUS_BOUND: f32 = WORLD_CUBE_HALF - 3.0;

/// Per-camera fly-through state: the smoothed dolly velocity (`vel`, world-units/s
/// along the view axis) and the input-driven target it eases toward.
#[derive(Resource, Default)]
pub(crate) struct NavState {
    vel: f32,
    target_vel: f32,
}

/// Tracks per-frame two-finger touch state for the two dolly gestures:
/// - **Pinch**: spread-distance change → zoom (finger-apart = dolly back)
/// - **Two-finger scroll**: average-Y change → zoom (both fingers up = dolly forward)
///   Works like a trackwheel when both fingers move in the same direction.
#[derive(Resource, Default)]
struct TouchPinch {
    prev_dist: Option<f32>,
    prev_avg_y: Option<f32>,
}

/// Pixels-of-spread-change per frame → dolly gain for pinch.
const TOUCH_PINCH_GAIN: f32 = 0.03;
/// Pixels-of-average-Y-change per frame → dolly gain for two-finger scroll.
/// Screen-up is negative Y in Bevy window coords, so negate to get "up = forward".
const TOUCH_SCROLL_GAIN: f32 = 0.06;

/// Wire the §4.8 fly-through into the app: register [`NavState`] and the two
/// per-frame systems. Called from `render::run_app`.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<NavState>()
        .init_resource::<TouchPinch>()
        .add_systems(Update, (fly_through, update_near_clip));
}

/// Scroll-up / pinch-in dollies the panorbit rig forward along the view direction
/// (and past board centre); scroll-down / pinch-out reverses. See module docs.
#[allow(clippy::too_many_arguments)]
fn fly_through(
    time: Res<Time>,
    mut wheel: MessageReader<MouseWheel>,
    mut pinch: MessageReader<PinchGesture>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    touches: Res<Touches>,
    mut touch_pinch: ResMut<TouchPinch>,
    mut nav: ResMut<NavState>,
    mut cam: Query<(&mut PanOrbitCamera, &Transform), With<crate::render::MainCam>>,
    ui_buttons: Query<&Interaction, With<Button>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    // Both mouse buttons held = the §4.8 forward/back dolly (an alternative to the
    // two-finger / wheel path for mouse users). While both are down we suppress
    // panorbit so its orbit (left) + pan (right) don't fight the dolly.
    let dual = buttons.pressed(MouseButton::Left) && buttons.pressed(MouseButton::Right);

    // Collect this frame's "zoom-in is forward" input. Wheel-up (+y) and pinch-in
    // (+magnification) both read as forward; two-finger trackpad drags arrive as
    // pixel-unit wheel deltas, so they fly through too.
    let mut scroll = 0.0;
    for ev in wheel.read() {
        scroll += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / WHEEL_PIXELS_PER_LINE,
        };
    }
    for ev in pinch.read() {
        scroll += ev.0 * PINCH_GAIN;
    }
    // Both-buttons drag: up = forward (screen-space +y is down, so negate).
    if dual {
        scroll += -motion.delta.y * DUAL_MOUSE_GAIN;
    }
    // Two-finger touch gestures (mobile / touch screens, where PinchGesture
    // doesn't fire). We read two inputs simultaneously from the same two fingers:
    //
    //  • Pinch  — spread-distance change.  Fingers apart = dolly back.
    //  • Scroll — average-Y change.  Both fingers up = dolly forward, like a
    //             trackwheel.  Negate because Bevy window Y grows downward.
    let active: Vec<Vec2> = touches.iter().take(2).map(|t| t.position()).collect();
    if active.len() >= 2 {
        let dist = active[0].distance(active[1]);
        let avg_y = (active[0].y + active[1].y) * 0.5;

        if let Some(prev) = touch_pinch.prev_dist {
            scroll += (dist - prev) * TOUCH_PINCH_GAIN;
        }
        if let Some(prev_y) = touch_pinch.prev_avg_y {
            // avg_y increases when fingers move DOWN (screen +Y = down).
            // Down = dolly back = negative scroll; up = dolly forward = positive.
            scroll += -(avg_y - prev_y) * TOUCH_SCROLL_GAIN;
        }

        touch_pinch.prev_dist = Some(dist);
        touch_pinch.prev_avg_y = Some(avg_y);
    } else {
        touch_pinch.prev_dist = None;
        touch_pinch.prev_avg_y = None;
    }

    let Ok((mut orbit, transform)) = cam.single_mut() else {
        return;
    };
    // Disable panorbit while any UI button is held (e.g. slider drag) so the
    // mouse motion isn't interpreted as an orbit at the same time.
    let ui_pressed = ui_buttons
        .iter()
        .any(|i| *i == Interaction::Pressed);
    // Suppress panorbit's own orbit/pan while both buttons drive the dolly OR
    // while a UI element is being dragged.
    orbit.enabled = !dual && !ui_pressed;

    // Drive the desired velocity from input, clamp to the speed cap, then let it
    // coast back to rest when no new input arrives this frame.
    nav.target_vel =
        (nav.target_vel + scroll * WHEEL_LINE_GAIN).clamp(-FORWARD_SPEED_CAP, FORWARD_SPEED_CAP);
    nav.target_vel *= (-dt / COAST_TAU).exp();

    // Exponentially smooth the actual velocity toward the target (τ = SMOOTHING_TAU).
    let alpha = 1.0 - (-dt / SMOOTHING_TAU).exp();
    nav.vel += (nav.target_vel - nav.vel) * alpha;
    if nav.vel.abs() < 1.0e-4 {
        nav.vel = 0.0;
        return;
    }

    // Transform.forward() points camera → focus (Bevy forward is −Z). Translating
    // the focus along it moves the whole rig forward without disturbing orbit.
    let step = transform.forward().as_vec3() * (nav.vel * dt);
    let bound = Vec3::splat(FOCUS_BOUND);
    orbit.focus = (orbit.focus + step).clamp(-bound, bound);
    orbit.target_focus = (orbit.target_focus + step).clamp(-bound, bound);
}

/// Push the perspective near plane forward while inside the lattice so the cells
/// the camera is passing through stop occluding the interior ("faces flow behind
/// you"); rest at [`NEAR_OUTSIDE`] outside the volume.
fn update_near_clip(
    board: Res<BoardResource>,
    mut cam: Query<(&Transform, &mut Projection), With<crate::render::MainCam>>,
) {
    let Ok((transform, mut projection)) = cam.single_mut() else {
        return;
    };
    let Projection::Perspective(persp) = projection.as_mut() else {
        return;
    };

    // Board AABB is centred on the origin and spans ±N/2 per axis (§4.8; cell
    // centres run coord−(N−1)/2, shells radius SHELL_RADIUS).
    let half = board.0.board.n as f32 / 2.0;
    let p = transform.translation;
    let inside = p.x.abs() <= half + AABB_EPS
        && p.y.abs() <= half + AABB_EPS
        && p.z.abs() <= half + AABB_EPS;

    // Inside: clip the slab the camera currently occupies (≈ the cell at the
    // viewpoint plus its near neighbour) so the deeper interior reads, bounded so
    // the far half of the board is always visible. Outside: ordinary near.
    //
    // NOTE: §4.8's prose says `near = camera_to_facing_AABB_face + 0.05`. Taken
    // literally that pushes the near plane to the *far* wall and clips the entire
    // interior — the opposite of the stated goal ("see through to the interior").
    // The `camera.nearClipOffset` knob ("offset from camera position") is the
    // precise intent, so the slab is measured from the camera, not the far face.
    let near = if inside {
        (CELL_PITCH + NEAR_CLIP_OFFSET).clamp(NEAR_OUTSIDE, (half - AABB_EPS).max(NEAR_OUTSIDE))
    } else {
        NEAR_OUTSIDE
    };

    if (persp.near - near).abs() > 1.0e-4 {
        persp.near = near;
    }
}
