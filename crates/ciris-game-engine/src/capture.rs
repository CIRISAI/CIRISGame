//! Dev screenshot capture (native only). A windowed run has no way to be inspected
//! from the agent / CI environment (no display), so this grabs PNG frames a few
//! seconds after launch — once the screensaver has placed a few cells and the
//! liquid pipes are flowing — and on demand via F12. Frames land in `screenshots/`
//! at the repo root and the absolute path is logged.
//!
//! Feature-gated behind `render` and compiled only off-wasm: [`save_to_disk`]
//! writes a file on native but triggers a browser download on wasm, which we do
//! not want for the deployed artifacts.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

/// Seconds-after-launch at which to auto-capture, and a running frame counter.
#[derive(Resource)]
struct CaptureState {
    /// Pending auto-capture times (s), ascending; popped as each fires.
    pending: Vec<f32>,
    /// Monotonic counter for the saved filenames.
    counter: u32,
}

/// Register the keypress capture system. F12 takes a screenshot; auto-capture
/// is disabled (it was useful for CI inspection but is now just noise).
pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(CaptureState {
        pending: vec![],
        counter: 0,
    })
    .add_systems(Update, (auto_capture, key_capture));
}

/// Fire the scheduled auto-captures once their launch-relative time has passed.
fn auto_capture(time: Res<Time>, mut state: ResMut<CaptureState>, mut commands: Commands) {
    let now = time.elapsed_secs();
    while state.pending.first().is_some_and(|&t| now >= t) {
        state.pending.remove(0);
        capture(&mut commands, &mut state.counter);
    }
}

/// Capture on F12.
fn key_capture(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CaptureState>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::F12) {
        capture(&mut commands, &mut state.counter);
    }
}

/// Spawn a one-shot screenshot of the primary window, saved to
/// `screenshots/run-NNN.png`, and log the absolute path.
fn capture(commands: &mut Commands, counter: &mut u32) {
    let dir = std::path::Path::new("screenshots");
    if let Err(e) = std::fs::create_dir_all(dir) {
        warn!("screenshot: could not create {}: {e}", dir.display());
        return;
    }
    let path = dir.join(format!("run-{:03}.png", *counter));
    *counter += 1;
    let shown = std::fs::canonicalize(&path)
        .or_else(|_| std::fs::canonicalize(dir).map(|d| d.join(path.file_name().unwrap())))
        .unwrap_or_else(|_| path.clone());
    info!("screenshot: capturing -> {}", shown.display());
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}
